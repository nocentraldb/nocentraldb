//! The handler of one invocation: an API Gateway WebSocket event in, the
//! relay core's answers out through the connection API. It checks nothing
//! itself: every request goes through [`ncdb_relay::Relay`], and the
//! signed challenge through [`ncdb_relay::auth`], as in the WebSocket
//! server. What the server holds in a connection's task, the step of its
//! opening and its subscriptions, lives in [`Connections`] between
//! invocations.

use std::time::Instant;

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{RelayStore, Token, UploadId};
use ncdb_relay::wire::{self, Cursor, Frame, MAX_MESSAGE, MAX_PUSH, Request, VERSIONS};
use ncdb_relay::{ErrorCode, PushFrame, Pushed, Relay, auth, page};

use crate::connections::{
    Connection, ConnectionApi, Connections, MAX_CONNECTION_AGE, Posted, Stage, Subscription,
};
use crate::event::{Body, Event, Route};
use crate::log::Line;

/// The most entries one page of a pull or a subscription reads, as in the
/// WebSocket server.
pub const PAGE: u64 = 256;

/// How the deployment makes the URLs of large changes from their tokens
/// (RFC 0007, "Large changes"): presigned S3 URLs on AWS, the harness's own
/// listener in tests.
pub trait Urls: Send + Sync {
    /// The URL an upload's frame goes to.
    fn upload(&self, token: &Token) -> String;
    /// The URL an entry's frame comes from.
    fn download(&self, token: &Token) -> String;
}

/// The relay as one Lambda function: the relay core over a backend `S`,
/// the open connections `C`, and the connection API `A`.
pub struct Handler<S, C, A, U> {
    relay: Relay<S>,
    relay_id: [u8; 16],
    connections: C,
    api: A,
    urls: U,
}

/// The invocation's state: the event's connection, and its log line.
struct Call<'a> {
    id: &'a str,
    line: Line,
}

impl<S, C, A, U> Handler<S, C, A, U>
where
    S: RelayStore,
    C: Connections,
    A: ConnectionApi,
    U: Urls,
{
    /// A handler over `relay`, whose deployment has the stable id
    /// `relay_id`.
    pub fn new(relay: Relay<S>, relay_id: [u8; 16], connections: C, api: A, urls: U) -> Self {
        Handler {
            relay,
            relay_id,
            connections,
            api,
            urls,
        }
    }

    /// The relay core.
    pub fn relay(&self) -> &Relay<S> {
        &self.relay
    }

    /// The open connections.
    pub fn connections(&self) -> &C {
        &self.connections
    }

    /// The connection API.
    pub fn api(&self) -> &A {
        &self.api
    }

    /// Handles one event; its log line.
    pub async fn handle(&self, event: Event) -> Line {
        let started = Instant::now();
        let mut call = Call {
            id: &event.connection,
            line: Line::new(event.route, &event.connection, event.at),
        };
        call.line.bytes_in = event.body.len();
        match event.route {
            Route::Connect => {
                let connection = Connection {
                    stage: Stage::Hello,
                    connected_at: event.connected_at,
                    expires_at: event.connected_at.saturating_add(MAX_CONNECTION_AGE),
                };
                if self.connections.put(call.id, connection).await.is_err() {
                    call.line.codes.push(ErrorCode::Unavailable);
                }
            }
            Route::Disconnect => {
                if self.connections.remove(call.id).await.is_err() {
                    call.line.codes.push(ErrorCode::Unavailable);
                }
            }
            Route::Message => self.message(&mut call, event.body).await,
        }
        call.line.ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        call.line
    }

    /// Posts `bytes` to `id`; a gone connection is forgotten.
    async fn post_to(&self, call: &mut Call<'_>, id: &str, bytes: Vec<u8>) -> bool {
        debug_assert!(bytes.len() <= MAX_MESSAGE);
        let len = bytes.len();
        match self.api.post(id, bytes).await {
            Ok(Posted::Sent) => {
                call.line.posts += 1;
                call.line.bytes_out += len;
                true
            }
            Ok(Posted::Gone) => {
                let _ = self.connections.remove(id).await;
                false
            }
            Err(_) => false,
        }
    }

    /// Posts `bytes` to the event's own connection.
    async fn send(&self, call: &mut Call<'_>, bytes: Vec<u8>) {
        let id = call.id;
        self.post_to(call, id, bytes).await;
    }

    /// Sends an error and keeps the connection open.
    async fn fail(&self, call: &mut Call<'_>, req: Option<u64>, code: ErrorCode) {
        call.line.codes.push(code);
        self.send(call, wire::error(req, code)).await;
    }

    /// Sends an error and closes the connection.
    async fn refuse(&self, call: &mut Call<'_>, req: Option<u64>, code: ErrorCode) {
        self.fail(call, req, code).await;
        let _ = self.api.close(call.id).await;
        let _ = self.connections.remove(call.id).await;
    }

    /// Sends an answer, or the relay core's error. An answer that does not
    /// fit into one message is `too_large`: RFC 0007 pages none of these.
    async fn answer(&self, call: &mut Call<'_>, req: u64, answer: Result<Vec<u8>, ErrorCode>) {
        match answer {
            Ok(bytes) if bytes.len() <= MAX_MESSAGE => self.send(call, bytes).await,
            Ok(_) => self.fail(call, Some(req), ErrorCode::TooLarge).await,
            Err(code) => self.fail(call, Some(req), code).await,
        }
    }

    async fn message(&self, call: &mut Call<'_>, body: Body) {
        let connection = match self.connections.get(call.id).await {
            Ok(Some(connection)) => connection,
            // A message on a connection the relay holds no record of: its
            // `$connect` was missed or its record expired.
            Ok(None) => return self.refuse(call, None, ErrorCode::Unauthenticated).await,
            Err(_) => return self.fail(call, None, ErrorCode::Unavailable).await,
        };
        if let Stage::Member(member) = connection.stage {
            call.line.member = Some(member);
        }
        let request = match body {
            Body::Binary(bytes) => wire::decode(&bytes),
            Body::Text(_) | Body::None => Err(wire::Malformed { req: None }),
        };
        let request = match request {
            Ok(request) => request,
            Err(malformed) => {
                return match connection.stage {
                    Stage::Member(_) => self.fail(call, malformed.req, ErrorCode::Malformed).await,
                    _ => {
                        self.refuse(call, malformed.req, ErrorCode::Unauthenticated)
                            .await
                    }
                };
            }
        };
        call.line.kind = Some(kind(&request));
        match (connection.stage, request) {
            (Stage::Hello, Request::Hello { versions }) => {
                self.hello(call, connection, &versions).await
            }
            (Stage::Auth { nonce }, Request::Auth { key, signature }) => {
                match auth::verify(&self.relay_id, &nonce, &key, &signature) {
                    Ok(member) => self.welcome(call, connection, member).await,
                    Err(code) => self.refuse(call, None, code).await,
                }
            }
            (Stage::Member(member), request) => self.request(call, member, request).await,
            (_, request) => {
                self.refuse(call, request.req(), ErrorCode::Unauthenticated)
                    .await
            }
        }
    }

    /// Answers `hello` with a challenge, or refuses a member that speaks
    /// none of this relay's versions.
    async fn hello(&self, call: &mut Call<'_>, connection: Connection, versions: &[u64]) {
        let Some(version) = VERSIONS.into_iter().rev().find(|v| versions.contains(v)) else {
            return self.refuse(call, None, ErrorCode::UnsupportedVersion).await;
        };
        let mut nonce = [0u8; 32];
        if getrandom::fill(&mut nonce).is_err() {
            return self.refuse(call, None, ErrorCode::Unavailable).await;
        }
        let stage = Stage::Auth { nonce };
        if self
            .connections
            .put(
                call.id,
                Connection {
                    stage,
                    ..connection
                },
            )
            .await
            .is_err()
        {
            return self.refuse(call, None, ErrorCode::Unavailable).await;
        }
        let limits = [MAX_MESSAGE, MAX_PUSH, self.relay.config().max_frame];
        self.send(
            call,
            wire::challenge(version, &self.relay_id, &nonce, limits),
        )
        .await;
    }

    /// The connection acts as `member` from now on.
    async fn welcome(&self, call: &mut Call<'_>, connection: Connection, member: MemberId) {
        call.line.member = Some(member);
        let store = self.relay.store();
        let (Ok(epoch), Ok(spaces)) = (store.epoch().await, store.members_of(member).await) else {
            return self.refuse(call, None, ErrorCode::Unavailable).await;
        };
        let stage = Stage::Member(member);
        if self
            .connections
            .put(
                call.id,
                Connection {
                    stage,
                    ..connection
                },
            )
            .await
            .is_err()
        {
            return self.refuse(call, None, ErrorCode::Unavailable).await;
        }
        let spaces: Vec<[u8; 16]> = spaces.iter().map(|s| s.0).collect();
        self.send(call, wire::welcome(&member.0, &epoch.0, &spaces))
            .await;
    }

    /// A request of an authenticated member.
    async fn request(&self, call: &mut Call<'_>, member: MemberId, request: Request) {
        let relay = &self.relay;
        match request {
            Request::Push { req, space, frames } => {
                self.push(call, member, req, space, frames).await
            }
            Request::Pull(cursor) => {
                let Cursor {
                    req,
                    space,
                    epoch,
                    after,
                    limit,
                } = cursor;
                let url = |token: &Token| self.urls.download(token);
                let limit = limit.min(PAGE);
                match page::read(relay, &url, member, Some(req), space, epoch, after, limit).await {
                    Ok(page) => self.send(call, page.bytes).await,
                    Err(code) => self.fail(call, Some(req), code).await,
                }
            }
            Request::Subscribe(cursor) => self.subscribe(call, member, cursor).await,
            Request::Unsubscribe { space, .. } => {
                if self.connections.unsubscribe(call.id, space).await.is_err() {
                    call.line.codes.push(ErrorCode::Unavailable);
                }
            }
            Request::Upload { req, space, size } => {
                let begun = relay.begin_upload(member, space, size).await;
                let answer =
                    begun.map(|(id, token)| wire::upload(req, &id.0, &self.urls.upload(&token)));
                self.answer(call, req, answer).await
            }
            Request::Keys { req, space } => {
                let keys = relay.keys(member, space).await;
                let answer = keys.map(|keys| wire::keys(req, space, &keys));
                self.answer(call, req, answer).await
            }
            Request::PersonKeys { req, space } => {
                let records = relay.person_keys(member, space).await;
                let answer = records.map(|records| wire::person_keys(req, space, &records));
                self.answer(call, req, answer).await
            }
            Request::PutPersonKeys {
                req,
                space,
                records,
            } => {
                let put = relay.put_person_keys(member, space, records).await;
                self.answer(call, req, put.map(|()| wire::ok(req))).await
            }
            Request::Ping { req } => self.send(call, wire::pong(req)).await,
            Request::Hello { .. } | Request::Auth { .. } => {
                self.fail(call, None, ErrorCode::Malformed).await
            }
        }
    }

    /// Checks and appends the frames, answers `pushed`, tells the members
    /// the push made current with `added`, and delivers what it appended
    /// to the space's subscribers. Delivery is best-effort (RFC 0007,
    /// "Delivery to subscribers").
    async fn push(
        &self,
        call: &mut Call<'_>,
        member: MemberId,
        req: u64,
        space: SpaceId,
        frames: Vec<Frame>,
    ) {
        let frames = frames
            .into_iter()
            .map(|frame| match frame {
                Frame::Change(bytes) => PushFrame::Bytes(bytes),
                Frame::Upload(id) => match <[u8; 16]>::try_from(id) {
                    Ok(id) => PushFrame::Upload(UploadId(id)),
                    // An empty frame is malformed too: it decodes to no change.
                    Err(_) => PushFrame::Bytes(Vec::new()),
                },
            })
            .collect();
        let checked = self.relay.push_frames(member, space, frames).await;
        call.line
            .codes
            .extend(checked.results.iter().filter_map(|result| match result {
                Pushed::Rejected(code) => Some(*code),
                _ => None,
            }));
        self.send(call, wire::pushed(req, &checked.results)).await;
        for added in &checked.added {
            let Ok(connections) = self.connections.of_member(*added).await else {
                continue;
            };
            for id in connections {
                self.post_to(call, &id, wire::added(space)).await;
            }
        }
        let accepted = checked.results.iter().filter_map(|result| match result {
            Pushed::Accepted(position) => Some(*position),
            _ => None,
        });
        let (first, last) = accepted.fold((u64::MAX, 0), |(lo, hi), p| (lo.min(p), hi.max(p)));
        if last > 0 {
            self.deliver(call, space, first - 1, last).await;
        }
    }

    /// Posts the entries of `space` after `after`, up to `last`, to each of
    /// its subscribers, read as each subscriber's member, so a member that
    /// is no longer current reads nothing: its subscription ends with
    /// `removed_member`, as in the WebSocket server.
    async fn deliver(&self, call: &mut Call<'_>, space: SpaceId, after: u64, last: u64) {
        let Ok(subscribers) = self.connections.subscribers(space).await else {
            return;
        };
        let url = |token: &Token| self.urls.download(token);
        for (id, subscription) in subscribers {
            let mut cursor = after;
            while cursor < last {
                let read = page::read(
                    &self.relay,
                    &url,
                    subscription.member,
                    None,
                    space,
                    subscription.epoch,
                    cursor,
                    (last - cursor).min(PAGE),
                )
                .await;
                match read {
                    Ok(page) => {
                        let Some(end) = page.last else { break };
                        if !self.post_to(call, &id, page.bytes).await {
                            break;
                        }
                        cursor = end;
                    }
                    Err(code) => {
                        let code = match code {
                            ErrorCode::NotMember => ErrorCode::RemovedMember,
                            code => code,
                        };
                        call.line.codes.push(code);
                        self.post_to(call, &id, wire::error(Some(subscription.req), code))
                            .await;
                        let _ = self.connections.unsubscribe(&id, space).await;
                        break;
                    }
                }
            }
        }
    }

    /// Subscribes the connection, then posts the catch-up pages with the
    /// request id until the head, as the WebSocket server does; what a push
    /// appends meanwhile reaches it through [`Self::deliver`] too, and the
    /// member skips a position it already holds.
    async fn subscribe(&self, call: &mut Call<'_>, member: MemberId, cursor: Cursor) {
        let Cursor {
            req,
            space,
            epoch,
            mut after,
            ..
        } = cursor;
        let subscription = Subscription { member, req, epoch };
        if self
            .connections
            .subscribe(call.id, space, subscription)
            .await
            .is_err()
        {
            return self.fail(call, Some(req), ErrorCode::Unavailable).await;
        }
        let url = |token: &Token| self.urls.download(token);
        loop {
            let read = page::read(
                &self.relay,
                &url,
                member,
                Some(req),
                space,
                epoch,
                after,
                PAGE,
            );
            match read.await {
                Ok(page) => {
                    let end = page.last;
                    let head = page.head;
                    self.send(call, page.bytes).await;
                    match end {
                        Some(end) if end < head => after = end,
                        _ => return,
                    }
                }
                Err(code) => {
                    let _ = self.connections.unsubscribe(call.id, space).await;
                    return self.fail(call, Some(req), code).await;
                }
            }
        }
    }
}

/// The type of a request, as its message names it.
pub fn kind(request: &Request) -> &'static str {
    match request {
        Request::Hello { .. } => "hello",
        Request::Auth { .. } => "auth",
        Request::Push { .. } => "push",
        Request::Pull(_) => "pull",
        Request::Subscribe(_) => "subscribe",
        Request::Unsubscribe { .. } => "unsubscribe",
        Request::Upload { .. } => "upload",
        Request::Keys { .. } => "keys",
        Request::PersonKeys { .. } => "person-keys",
        Request::PutPersonKeys { .. } => "put-person-keys",
        Request::Ping { .. } => "ping",
    }
}
