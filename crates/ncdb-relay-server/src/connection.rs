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
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

use crate::hub::Hub;
use crate::subscription;
use crate::wire::{self, Cursor, Frame, Malformed, Request};
use crate::{MAX_FRAME, MAX_MESSAGE, MAX_PUSH, VERSIONS};

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
    /// Acting as this member.
    Member(MemberId),
}

/// Serves one TCP connection until it closes.
pub(crate) async fn serve<S: RelayStore + 'static>(stream: TcpStream, shared: Arc<Shared<S>>) {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE));
    let Ok(ws) = tokio_tungstenite::accept_async_with_config(stream, Some(config)).await else {
        return;
    };
    let (mut sink, mut incoming) = ws.split();
    let (out, mut outgoing) = mpsc::unbounded_channel::<Out>();
    let writer = tokio::spawn(async move {
        while let Some(next) = outgoing.recv().await {
            match next {
                Out::Message(bytes) => {
                    if sink.send(Message::Binary(bytes.into())).await.is_err() {
                        return;
                    }
                }
                Out::Close => {
                    let _ = sink.close().await;
                    return;
                }
            }
        }
    });
    let mut connection = Connection {
        shared,
        out,
        state: State::Hello,
        subscriptions: HashMap::new(),
    };
    while let Some(Ok(message)) = incoming.next().await {
        let open = match message {
            Message::Binary(bytes) => connection.receive(&bytes).await,
            Message::Text(_) => connection.malformed(Malformed { req: None }),
            Message::Close(_) => false,
            _ => true,
        };
        if !open {
            break;
        }
    }
    let _ = connection.out.send(Out::Close);
    drop(connection);
    let _ = writer.await;
}

struct Connection<S> {
    shared: Arc<Shared<S>>,
    out: mpsc::UnboundedSender<Out>,
    state: State,
    /// The running subscription of each space: a subscription lives as
    /// long as its connection.
    subscriptions: HashMap<SpaceId, JoinHandle<()>>,
}

impl<S> Drop for Connection<S> {
    fn drop(&mut self) {
        self.subscriptions.values().for_each(JoinHandle::abort);
    }
}

impl<S: RelayStore + 'static> Connection<S> {
    fn send(&self, bytes: Vec<u8>) {
        let _ = self.out.send(Out::Message(bytes));
    }

    /// Answers an error and keeps the connection open.
    fn fail(&self, req: Option<u64>, code: ErrorCode) -> bool {
        self.send(wire::error(req, code));
        true
    }

    /// Answers an error and closes the connection.
    fn refuse(&self, req: Option<u64>, code: ErrorCode) -> bool {
        self.send(wire::error(req, code));
        false
    }

    /// Sends an answer, or the error of the relay core. An answer that does
    /// not fit into one message is `too_large`: RFC 0007 pages none of
    /// these answers.
    fn answer(&self, req: u64, answer: Result<Vec<u8>, ErrorCode>) -> bool {
        match answer {
            Ok(bytes) if bytes.len() <= MAX_MESSAGE => {
                self.send(bytes);
                true
            }
            Ok(_) => self.fail(Some(req), ErrorCode::TooLarge),
            Err(code) => self.fail(Some(req), code),
        }
    }

    fn malformed(&self, malformed: Malformed) -> bool {
        match self.state {
            State::Member(_) => self.fail(malformed.req, ErrorCode::Malformed),
            _ => self.refuse(malformed.req, ErrorCode::Unauthenticated),
        }
    }

    /// Handles one message; false when the connection closes.
    async fn receive(&mut self, bytes: &[u8]) -> bool {
        let request = match wire::decode(bytes) {
            Ok(request) => request,
            Err(malformed) => return self.malformed(malformed),
        };
        match (&self.state, request) {
            (State::Hello, Request::Hello { versions }) => self.hello(&versions),
            (State::Auth { nonce }, Request::Auth { key, signature }) => {
                match auth::verify(&self.shared.relay_id, nonce, &key, &signature) {
                    Ok(member) => self.welcome(member).await,
                    Err(code) => self.refuse(None, code),
                }
            }
            (State::Member(member), request) => {
                let member = *member;
                self.request(member, request).await
            }
            (_, request) => self.refuse(request.req(), ErrorCode::Unauthenticated),
        }
    }

    /// Answers `hello` with a challenge, or refuses a member that speaks
    /// none of this server's versions.
    fn hello(&mut self, versions: &[u64]) -> bool {
        let Some(version) = VERSIONS.into_iter().rev().find(|v| versions.contains(v)) else {
            return self.refuse(None, ErrorCode::UnsupportedVersion);
        };
        let mut nonce = [0u8; 32];
        if getrandom::fill(&mut nonce).is_err() {
            return self.refuse(None, ErrorCode::Unavailable);
        }
        let limits = [MAX_MESSAGE, MAX_PUSH, MAX_FRAME];
        let relay_id = &self.shared.relay_id;
        self.send(wire::challenge(version, relay_id, &nonce, limits));
        self.state = State::Auth { nonce };
        true
    }

    /// A request of an authenticated member.
    async fn request(&mut self, member: MemberId, request: Request) -> bool {
        let relay = self.shared.relay.clone();
        match request {
            Request::Push { req, space, frames } => {
                let results = push(relay, member, space, frames).await;
                self.send(wire::pushed(req, &results));
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
                        self.send(bytes);
                        true
                    }
                    Err(code) => self.fail(Some(req), code),
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
            Request::Upload { req } => self.fail(Some(req), ErrorCode::TooLarge),
            Request::Keys { req, space } => {
                let keys = blocking(async move { relay.keys(member, space).await }).await;
                let answer = keys.map(|keys| wire::keys(req, space, &keys));
                self.answer(req, answer)
            }
            Request::PersonKeys { req, space } => {
                let records = blocking(async move { relay.person_keys(member, space).await }).await;
                let answer = records.map(|records| wire::person_keys(req, space, &records));
                self.answer(req, answer)
            }
            Request::PutPersonKeys {
                req,
                space,
                records,
            } => {
                let put =
                    blocking(async move { relay.put_person_keys(member, space, records).await })
                        .await;
                self.answer(req, put.map(|()| wire::ok(req)))
            }
            Request::Ping { req } => {
                self.send(wire::pong(req));
                true
            }
            Request::Hello { .. } | Request::Auth { .. } => self.fail(None, ErrorCode::Malformed),
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
            return self.refuse(None, ErrorCode::Unavailable);
        };
        let spaces: Vec<[u8; 16]> = spaces.iter().map(|s| s.0).collect();
        self.send(wire::welcome(&member.0, &epoch.0, &spaces));
        self.state = State::Member(member);
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

/// The results of a push: the relay core checks and appends the frames, in
/// order; a reference to an upload is `too_large`, since this server
/// accepts no upload.
async fn push<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    member: MemberId,
    space: SpaceId,
    frames: Vec<Frame>,
) -> Vec<Pushed> {
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
    let mut checked = checked.into_iter();
    uploads
        .into_iter()
        .map(|upload| match upload {
            true => Pushed::Rejected(ErrorCode::TooLarge),
            false => checked
                .next()
                .unwrap_or(Pushed::Rejected(ErrorCode::Unavailable)),
        })
        .collect()
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
