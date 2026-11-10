//! One member's connection (RFC 0007, "Opening a connection"): hello,
//! challenge, auth, welcome, then requests until either side closes. Every
//! check goes through the relay core; this module only reads and writes
//! messages.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Pushed, Relay, auth};
use tokio::net::TcpStream;
use tokio::sync::{OwnedSemaphorePermit, mpsc};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::hub::Hub;
use crate::subscription;
use crate::wire::{self, Cursor, Frame, Malformed, Request};
use crate::{MAX_FRAME, MAX_MESSAGE, MAX_PUSH, QUEUE, VERSIONS};

/// The most entries the server reads for one page of a pull; a page holds
/// fewer when they do not fit into one message.
pub(crate) const PAGE: u64 = 256;

/// What every connection of one server shares.
pub(crate) struct Shared<S> {
    pub relay: Arc<Relay<S>>,
    pub relay_id: [u8; 16],
    pub hub: Hub,
    /// How long writing one message may make no progress.
    pub write_timeout: std::time::Duration,
    /// How long each step of the opening handshake may take.
    pub handshake_timeout: std::time::Duration,
}

/// What the writer task sends: a message, or a close after the messages
/// before it.
pub(crate) enum Out {
    Message(Vec<u8>),
    Close,
}

/// Where a connection stands in its opening.
enum State {
    /// Waiting for `hello`.
    Hello,
    /// Waiting for `auth` to this nonce.
    Auth { nonce: [u8; 32] },
    /// Acting as this member, listed in the hub under this id.
    Member(MemberId, u64),
}

/// Serves one TCP connection until it closes.
///
/// Its outgoing queue holds at most [`QUEUE`] messages: whoever produces a
/// message waits for room, so a subscription reads the log only as fast as
/// the member takes its entries. When writing one message makes no
/// progress for the server's write timeout, the writer gives up and the
/// connection closes (RFC 0007, "Liveness"); the member resumes from its
/// cursors. Each step of the opening (the WebSocket upgrade, `hello`, and
/// `auth` after the challenge) closes the connection once it takes longer
/// than the server's handshake timeout, counted from the step's start, so
/// a peer that stops halfway holds no task for long.
///
/// `handshake` is the connection's place among the server's handshakes; it
/// is given back when the member is sent `welcome`.
pub(crate) async fn serve<S: RelayStore + 'static>(
    stream: TcpStream,
    shared: Arc<Shared<S>>,
    handshake: OwnedSemaphorePermit,
) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let step_timeout = shared.handshake_timeout;
    let upgrade = tokio_tungstenite::accept_async_with_config(stream, Some(config));
    let Ok(Ok(ws)) = tokio::time::timeout(step_timeout, upgrade).await else {
        return;
    };
    let (mut sink, mut incoming) = ws.split();
    let (out, mut outgoing) = mpsc::channel::<Out>(QUEUE);
    let timeout = shared.write_timeout;
    let mut writer = tokio::spawn(async move {
        while let Some(next) = outgoing.recv().await {
            match next {
                Out::Message(bytes) => {
                    let sent = sink.send(Message::Binary(bytes.into()));
                    if !matches!(tokio::time::timeout(timeout, sent).await, Ok(Ok(()))) {
                        return;
                    }
                }
                Out::Close => {
                    let _ = tokio::time::timeout(timeout, sink.close()).await;
                    return;
                }
            }
        }
    });
    let mut connection = Connection {
        shared,
        out,
        state: State::Hello,
        handshake: Some(handshake),
        subscriptions: HashMap::new(),
    };
    let mut writing = true;
    // The deadline of the current step of the opening, none once the
    // connection acts as a member.
    let mut step = connection.step();
    let mut deadline = Some(Instant::now() + step_timeout);
    loop {
        let message = tokio::select! {
            message = incoming.next() => message,
            _ = &mut writer => {
                writing = false;
                break;
            }
            () = expires(deadline) => break,
        };
        let open = match message {
            Some(Ok(Message::Binary(bytes))) => connection.receive(&bytes).await,
            Some(Ok(Message::Text(_))) => connection.malformed(Malformed { req: None }).await,
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => false,
            Some(Ok(_)) => true,
        };
        if !open {
            break;
        }
        if connection.step() != step {
            step = connection.step();
            deadline = (step != Step::Member).then(|| Instant::now() + step_timeout);
        }
    }
    let out = connection.out.clone();
    drop(connection);
    if writing {
        let _ = out.send(Out::Close).await;
        drop(out);
        let _ = writer.await;
    }
}

/// Waits until `deadline`, or forever when there is none.
async fn expires(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => tokio::time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// The steps of a connection's opening, without their data.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Hello,
    Auth,
    Member,
}

struct Connection<S> {
    shared: Arc<Shared<S>>,
    out: mpsc::Sender<Out>,
    state: State,
    /// The connection's place among the server's handshakes, until its
    /// member is sent `welcome`.
    handshake: Option<OwnedSemaphorePermit>,
    /// The running subscription of each space: a subscription lives as
    /// long as its connection.
    subscriptions: HashMap<SpaceId, JoinHandle<()>>,
}

impl<S> Drop for Connection<S> {
    fn drop(&mut self) {
        self.subscriptions.values().for_each(JoinHandle::abort);
        if let State::Member(member, id) = self.state {
            self.shared.hub.unlist(member, id);
        }
    }
}

impl<S: RelayStore + 'static> Connection<S> {
    /// The step of the opening this connection stands at.
    fn step(&self) -> Step {
        match self.state {
            State::Hello => Step::Hello,
            State::Auth { .. } => Step::Auth,
            State::Member(..) => Step::Member,
        }
    }

    /// Queues a message, waiting for room in the queue.
    async fn send(&self, bytes: Vec<u8>) {
        let _ = self.out.send(Out::Message(bytes)).await;
    }

    /// Answers an error and keeps the connection open.
    async fn fail(&self, req: Option<u64>, code: ErrorCode) -> bool {
        self.send(wire::error(req, code)).await;
        true
    }

    /// Answers an error and closes the connection.
    async fn refuse(&self, req: Option<u64>, code: ErrorCode) -> bool {
        self.send(wire::error(req, code)).await;
        false
    }

    /// Sends an answer, or the error of the relay core. An answer that does
    /// not fit into one message is `too_large`: RFC 0007 pages none of
    /// these answers.
    async fn answer(&self, req: u64, answer: Result<Vec<u8>, ErrorCode>) -> bool {
        match answer {
            Ok(bytes) if bytes.len() <= MAX_MESSAGE => {
                self.send(bytes).await;
                true
            }
            Ok(_) => self.fail(Some(req), ErrorCode::TooLarge).await,
            Err(code) => self.fail(Some(req), code).await,
        }
    }

    async fn malformed(&self, malformed: Malformed) -> bool {
        match self.state {
            State::Member(..) => self.fail(malformed.req, ErrorCode::Malformed).await,
            _ => self.refuse(malformed.req, ErrorCode::Unauthenticated).await,
        }
    }

    /// Handles one message; false when the connection closes.
    async fn receive(&mut self, bytes: &[u8]) -> bool {
        let request = match wire::decode(bytes) {
            Ok(request) => request,
            Err(malformed) => return self.malformed(malformed).await,
        };
        match (&self.state, request) {
            (State::Hello, Request::Hello { versions }) => self.hello(&versions).await,
            (State::Auth { nonce }, Request::Auth { key, signature }) => {
                match auth::verify(&self.shared.relay_id, nonce, &key, &signature) {
                    Ok(member) => self.welcome(member).await,
                    Err(code) => self.refuse(None, code).await,
                }
            }
            (State::Member(member, id), request) => {
                let (member, id) = (*member, *id);
                self.request(member, id, request).await
            }
            (_, request) => self.refuse(request.req(), ErrorCode::Unauthenticated).await,
        }
    }

    /// Answers `hello` with a challenge, or refuses a member that speaks
    /// none of this server's versions.
    async fn hello(&mut self, versions: &[u64]) -> bool {
        let Some(version) = VERSIONS.into_iter().rev().find(|v| versions.contains(v)) else {
            return self.refuse(None, ErrorCode::UnsupportedVersion).await;
        };
        let mut nonce = [0u8; 32];
        if getrandom::fill(&mut nonce).is_err() {
            return self.refuse(None, ErrorCode::Unavailable).await;
        }
        let limits = [MAX_MESSAGE, MAX_PUSH, MAX_FRAME];
        let relay_id = &self.shared.relay_id;
        self.send(wire::challenge(version, relay_id, &nonce, limits))
            .await;
        self.state = State::Auth { nonce };
        true
    }

    /// A request of an authenticated member, on the connection listed in
    /// the hub as `id`.
    async fn request(&mut self, member: MemberId, id: u64, request: Request) -> bool {
        let relay = self.shared.relay.clone();
        match request {
            Request::Push { req, space, frames } => {
                let (results, added) = push(relay, member, space, frames).await;
                self.send(wire::pushed(req, &results)).await;
                if added.contains(&member) {
                    self.send(wire::added(space)).await;
                }
                self.shared.hub.added(space, &added, id);
                let appended = results.iter().filter_map(|result| match result {
                    Pushed::Accepted(position) => Some(*position),
                    _ => None,
                });
                if let Some(head) = appended.max() {
                    self.shared.hub.appended(space, head);
                }
                true
            }
            Request::Pull(cursor) => {
                let limit = u32::try_from(cursor.limit.min(PAGE)).unwrap_or(u32::MAX);
                let Cursor {
                    req,
                    space,
                    epoch,
                    after,
                    ..
                } = cursor;
                let pulled =
                    blocking(async move { relay.pull(member, space, epoch, after, limit).await })
                        .await;
                match pulled {
                    Ok(page) => {
                        let (bytes, _) = wire::entries(Some(req), space, &page.entries, page.head);
                        self.send(bytes).await;
                        true
                    }
                    Err(code) => self.fail(Some(req), code).await,
                }
            }
            Request::Subscribe(cursor) => {
                let space = cursor.space;
                let head = self.shared.hub.watch(space);
                let running = subscription::spawn(relay, member, cursor, head, self.out.clone());
                if let Some(previous) = self.subscriptions.insert(space, running) {
                    previous.abort();
                }
                true
            }
            Request::Unsubscribe { space, .. } => {
                if let Some(running) = self.subscriptions.remove(&space) {
                    running.abort();
                }
                true
            }
            // Large changes are left out: this server's largest frame fits
            // into one message, so it accepts no upload.
            Request::Upload { req } => self.fail(Some(req), ErrorCode::TooLarge).await,
            Request::Keys { req, space } => {
                let keys = blocking(async move { relay.keys(member, space).await }).await;
                let answer = keys.map(|keys| wire::keys(req, space, &keys));
                self.answer(req, answer).await
            }
            Request::PersonKeys { req, space } => {
                let records = blocking(async move { relay.person_keys(member, space).await }).await;
                let answer = records.map(|records| wire::person_keys(req, space, &records));
                self.answer(req, answer).await
            }
            Request::PutPersonKeys {
                req,
                space,
                records,
            } => {
                let put =
                    blocking(async move { relay.put_person_keys(member, space, records).await })
                        .await;
                self.answer(req, put.map(|()| wire::ok(req))).await
            }
            Request::Ping { req } => {
                self.send(wire::pong(req)).await;
                true
            }
            Request::Hello { .. } | Request::Auth { .. } => {
                self.fail(None, ErrorCode::Malformed).await
            }
        }
    }

    /// The connection acts as `member` from now on.
    async fn welcome(&mut self, member: MemberId) -> bool {
        let relay = self.shared.relay.clone();
        let read = blocking(async move {
            let epoch = relay.store().epoch().await?;
            let spaces = relay.store().members_of(member).await?;
            Ok::<_, ncdb_relay::store::StoreError>((epoch, spaces))
        })
        .await;
        let Ok((epoch, spaces)) = read else {
            return self.refuse(None, ErrorCode::Unavailable).await;
        };
        let spaces: Vec<[u8; 16]> = spaces.iter().map(|s| s.0).collect();
        self.handshake = None;
        self.send(wire::welcome(&member.0, &epoch.0, &spaces)).await;
        let id = self.shared.hub.list(member, self.out.clone());
        self.state = State::Member(member, id);
        true
    }
}

impl Request {
    /// The request id, if the request has one.
    fn req(&self) -> Option<u64> {
        match self {
            Request::Hello { .. } | Request::Auth { .. } => None,
            Request::Push { req, .. }
            | Request::Pull(Cursor { req, .. })
            | Request::Subscribe(Cursor { req, .. })
            | Request::Unsubscribe { req, .. }
            | Request::Upload { req }
            | Request::Keys { req, .. }
            | Request::PersonKeys { req, .. }
            | Request::PutPersonKeys { req, .. }
            | Request::Ping { req } => Some(*req),
        }
    }
}

/// The results of a push, and the members it made current: the relay core
/// checks and appends the frames, in order; a reference to an upload is
/// `too_large`, since this server accepts no upload.
async fn push<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    member: MemberId,
    space: SpaceId,
    frames: Vec<Frame>,
) -> (Vec<Pushed>, Vec<MemberId>) {
    let uploads: Vec<bool> = frames
        .iter()
        .map(|f| matches!(f, Frame::Upload(_)))
        .collect();
    let changes = frames
        .into_iter()
        .filter_map(|frame| match frame {
            Frame::Change(bytes) => Some(bytes),
            Frame::Upload(_) => None,
        })
        .collect();
    let checked = blocking(async move { relay.push(member, space, changes).await }).await;
    let mut results = checked.results.into_iter();
    let results = uploads
        .into_iter()
        .map(|upload| match upload {
            true => Pushed::Rejected(ErrorCode::TooLarge),
            false => results
                .next()
                .unwrap_or(Pushed::Rejected(ErrorCode::Unavailable)),
        })
        .collect();
    (results, checked.added)
}

/// Runs a future of the relay core on a thread that may block: the SQLite
/// backend blocks the thread that polls it (`ncdb_relay::sqlite`).
pub(crate) async fn blocking<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || handle.block_on(future))
        .await
        .expect("a relay call does not panic")
}
