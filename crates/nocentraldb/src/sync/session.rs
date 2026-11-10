//! One connection to the relay (RFC 0007, "Opening a connection" and
//! "Island mode"): authenticate, then for every space pull from its
//! cursor to the head, push what the relay has not confirmed in local
//! order, and subscribe from the new cursor; then fold live entries as
//! they arrive and push each new write.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use ncdb_core::ids::SpaceId;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::wire::{self, Answer};
use super::{Connection, apply};
use crate::db::{Inner, lock};

/// A member sends `ping` after this long without a message (RFC 0007,
/// "Liveness"). The tests shorten this and every bound below.
const IDLE: Duration = if cfg!(test) {
    Duration::from_millis(300)
} else {
    Duration::from_secs(5 * 60)
};

/// The longest the WebSocket handshake may take.
const CONNECT: Duration = if cfg!(test) {
    Duration::from_millis(500)
} else {
    Duration::from_secs(30)
};

/// The longest the relay may take to answer a request, `ping` included;
/// a relay that does not answer in time is a lost connection.
const ANSWER: Duration = if cfg!(test) {
    Duration::from_millis(500)
} else {
    Duration::from_secs(30)
};

/// How a session ended.
pub(super) enum End {
    /// The database was dropped: syncing ends.
    Dropped,
    /// The connection was lost or the relay ended it; the member
    /// reconnects. `progress` says whether the session synced every space
    /// once, so that the backoff starts over; `retry_after` is how long the
    /// relay asked the member to wait (RFC 0007, `rate_limited`).
    Lost {
        progress: bool,
        retry_after: Option<Duration>,
    },
}

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Session<'a> {
    ws: Ws,
    db: &'a Weak<Mutex<Inner>>,
    epoch: [u8; 16],
    next_req: u64,
    /// The subscriptions, by the request id that opened them.
    subscriptions: BTreeMap<u64, SpaceId>,
    subscribed: BTreeSet<SpaceId>,
    /// Spaces whose live entries left a gap, to pull again.
    gaps: BTreeSet<SpaceId>,
    /// Whether every space was pulled, pushed and subscribed once.
    progress: bool,
}

/// The connection ended. An error code that stops syncing is reported by
/// [`Session::failed`]; whether the session made progress is filled in
/// when it ends.
fn lost() -> End {
    End::Lost {
        progress: false,
        retry_after: None,
    }
}

/// Runs `f` on the database, unless it was dropped.
fn with<R>(db: &Weak<Mutex<Inner>>, f: impl FnOnce(&mut Inner) -> R) -> Result<R, End> {
    let inner: Arc<Mutex<Inner>> = db.upgrade().ok_or(End::Dropped)?;
    let mut guard = lock(&inner);
    Ok(f(&mut guard))
}

/// One connection to the relay at `url`, served until it ends.
pub(super) async fn session(
    db: &Weak<Mutex<Inner>>,
    url: &str,
    wake: &mut watch::Receiver<()>,
) -> End {
    // RFC 0007's largest message, on every backend; a larger one ends the
    // connection instead of being read.
    let config = WebSocketConfig::default()
        .max_message_size(Some(wire::MAX_MESSAGE))
        .max_frame_size(Some(wire::MAX_MESSAGE));
    let connect = tokio_tungstenite::connect_async_with_config(url, Some(config), false);
    let Ok(Ok((ws, _))) = tokio::time::timeout(CONNECT, connect).await else {
        return lost();
    };
    let mut session = Session {
        ws,
        db,
        epoch: [0; 16],
        next_req: 0,
        subscriptions: BTreeMap::new(),
        subscribed: BTreeSet::new(),
        gaps: BTreeSet::new(),
        progress: false,
    };
    if let Err(end) = session.authenticate().await {
        return end;
    }
    match session.serve(wake).await.err().unwrap_or(lost()) {
        End::Lost { retry_after, .. } => End::Lost {
            progress: session.progress,
            retry_after,
        },
        End::Dropped => End::Dropped,
    }
}

impl Session<'_> {
    /// hello, challenge, auth, welcome.
    async fn authenticate(&mut self) -> Result<(), End> {
        self.send(wire::hello()).await?;
        let (relay_id, nonce) = match self.answer_in_time().await? {
            Answer::Challenge { relay_id, nonce } => (relay_id, nonce),
            Answer::Error { code, .. } => return Err(self.failed(code)),
            _ => return Err(lost()),
        };
        let digest = ncdb_relay::auth::digest(&relay_id, &nonce);
        let auth = with(self.db, |inner| {
            let public = inner.member.key().public();
            let signature = SigningKey::from_bytes(inner.member.signing()).sign(&digest);
            wire::auth(public.signing, public.agreement, signature.to_bytes())
        })?;
        self.send(auth).await?;
        let (epoch, spaces) = match self.answer_in_time().await? {
            Answer::Welcome { epoch, spaces } => (epoch, spaces),
            Answer::Error { code, .. } => return Err(self.failed(code)),
            _ => return Err(lost()),
        };
        self.epoch = epoch;
        with(self.db, |inner| {
            apply::welcome(inner, epoch, &spaces)?;
            let sync = inner.sync.as_mut().expect("a database that syncs");
            sync.connection = Connection::Connected;
            sync.error = None;
            Ok(())
        })?
        .map_err(|_: ncdb_core::view::Error| lost())
    }

    /// Records an error the relay ended the connection with.
    fn failed(&self, code: String) -> End {
        let _ = with(self.db, |inner| {
            if let Some(sync) = inner.sync.as_mut() {
                sync.error = Some(code.clone());
            }
        });
        lost()
    }

    /// Syncs every space, then follows them until the connection ends.
    async fn serve(&mut self, wake: &mut watch::Receiver<()>) -> Result<(), End> {
        wake.mark_unchanged();
        let mut push = false;
        loop {
            let gaps = std::mem::take(&mut self.gaps);
            for space in with(self.db, |inner| apply::spaces(inner))? {
                if !self.subscribed.contains(&space) {
                    self.space(space).await?;
                } else if gaps.contains(&space) {
                    self.pull(space).await?;
                } else if push {
                    self.push(space).await?;
                }
            }
            push = false;
            self.progress = true;
            with(self.db, |inner| {
                if let Some(sync) = inner.sync.as_mut() {
                    sync.epoch_changed = false;
                }
            })?;
            tokio::select! {
                answer = self.recv() => self.live(answer?)?,
                changed = wake.changed() => match changed {
                    Ok(()) => push = true,
                    Err(_) => return Err(End::Dropped),
                },
                () = tokio::time::sleep(IDLE) => {
                    let req = self.req();
                    self.send(wire::ping(req)).await?;
                    self.ask(req).await?;
                }
            }
        }
    }

    /// Pull, push and subscribe one space (RFC 0007, "Island mode").
    async fn space(&mut self, space: SpaceId) -> Result<(), End> {
        if !self.pull(space).await? {
            return Ok(());
        }
        self.push(space).await?;
        let req = self.req();
        let after = with(self.db, |inner| apply::cursor(inner, space))?;
        self.send(wire::subscribe(req, space, self.epoch, after))
            .await?;
        self.subscriptions.insert(req, space);
        self.subscribed.insert(space);
        let answer = self.ask(req).await?;
        self.answer(space, answer)?;
        Ok(())
    }

    /// Pulls `space` from its cursor to the head. Returns whether the
    /// space is still synced.
    async fn pull(&mut self, space: SpaceId) -> Result<bool, End> {
        loop {
            let req = self.req();
            let after = with(self.db, |inner| apply::cursor(inner, space))?;
            self.send(wire::pull(req, space, self.epoch, after)).await?;
            match self.ask(req).await? {
                Answer::Entries { entries, head, .. } => {
                    let empty = entries.is_empty();
                    self.entries(space, entries)?;
                    let cursor = with(self.db, |inner| apply::cursor(inner, space))?;
                    if cursor >= head || empty {
                        return Ok(true);
                    }
                    // A page that does not move the cursor would be asked
                    // for again and again: the relay is not serving the
                    // range, so the session ends and backs off.
                    if cursor == after {
                        return Err(lost());
                    }
                }
                // A space the relay does not hold yet: the push of its
                // genesis creates it.
                Answer::Error { code, .. } if code == "unknown_space" => return Ok(true),
                answer => return self.answer(space, answer).map(|()| false),
            }
        }
    }

    /// Pushes every change of `space` the relay has not confirmed, in
    /// local order, in pushes that each fit one message.
    async fn push(&mut self, space: SpaceId) -> Result<(), End> {
        let unconfirmed =
            with(self.db, |inner| apply::unconfirmed(inner, space))?.map_err(|_| lost())?;
        let (ids, frames): (Vec<_>, Vec<_>) = unconfirmed.into_iter().unzip();
        let mut ids = ids.into_iter();
        for batch in wire::batches(frames) {
            let batch_ids: Vec<_> = ids.by_ref().take(batch.len()).collect();
            let req = self.req();
            self.send(wire::push(req, space, &batch)).await?;
            match self.ask(req).await? {
                Answer::Pushed { results, .. } => {
                    let stopped = with(self.db, |inner| {
                        apply::pushed(inner, space, &batch_ids, results)
                    })?
                    .map_err(|_| lost())?;
                    if stopped.is_some() {
                        return Err(lost());
                    }
                }
                answer => return self.answer(space, answer),
            }
        }
        Ok(())
    }

    /// Handles an answer about `space` that is not the one asked for: an
    /// error, or entries of a subscription.
    fn answer(&mut self, space: SpaceId, answer: Answer) -> Result<(), End> {
        match answer {
            Answer::Entries { entries, head, .. } => {
                self.entries(space, entries)?;
                let cursor = with(self.db, |inner| apply::cursor(inner, space))?;
                if cursor < head {
                    self.gaps.insert(space);
                } else {
                    with(self.db, |inner| apply::caught_up(inner, space))?;
                }
                Ok(())
            }
            Answer::Error {
                code, retry_after, ..
            } => match code.as_str() {
                "not_member" | "removed_member" => {
                    with(self.db, |inner| apply::stop(inner, space, code))?;
                    Ok(())
                }
                // No subscription while the relay does not hold the space.
                "unknown_space" => Ok(()),
                _ => Err(End::Lost {
                    progress: false,
                    retry_after: retry_after.map(Duration::from_secs),
                }),
            },
            _ => Ok(()),
        }
    }

    /// A message the relay sent on its own: live entries, the end of a
    /// subscription, or `added`. Entries of a space this session did not
    /// subscribe to are ignored: a relay sends them only by mistake, and
    /// folding them would make that space one the database syncs. A space
    /// the member was added to is synced by [`Session::serve`] next, as a
    /// space named in `welcome` is.
    fn live(&mut self, answer: Answer) -> Result<(), End> {
        if let Answer::Added { space } = answer {
            if with(self.db, |inner| apply::added(inner, space))? {
                self.subscribed.remove(&space);
            }
            return Ok(());
        }
        let space = match &answer {
            Answer::Entries { space, .. } => {
                Some(*space).filter(|space| self.subscribed.contains(space))
            }
            other => other
                .req()
                .and_then(|req| self.subscriptions.get(&req).copied()),
        };
        match space {
            Some(space) => self.answer(space, answer),
            None => Ok(()),
        }
    }

    fn entries(&mut self, space: SpaceId, entries: Vec<wire::Entry>) -> Result<(), End> {
        let gap =
            with(self.db, |inner| apply::entries(inner, space, entries))?.map_err(|_| lost())?;
        if gap {
            self.gaps.insert(space);
        }
        Ok(())
    }

    fn req(&mut self) -> u64 {
        self.next_req += 1;
        self.next_req
    }

    async fn send(&mut self, message: Vec<u8>) -> Result<(), End> {
        self.ws
            .send(Message::Binary(message.into()))
            .await
            .map_err(|_| lost())
    }

    /// The answer to the request `req`; what arrives before it is handled
    /// as a message the relay sent on its own. A relay that does not
    /// answer within [`ANSWER`] is a lost connection.
    async fn ask(&mut self, req: u64) -> Result<Answer, End> {
        let answer = async {
            loop {
                let answer = self.recv().await?;
                if answer.req() == Some(req) {
                    return Ok(answer);
                }
                self.live(answer)?;
            }
        };
        tokio::time::timeout(ANSWER, answer)
            .await
            .unwrap_or_else(|_| Err(lost()))
    }

    /// The next message, within [`ANSWER`].
    async fn answer_in_time(&mut self) -> Result<Answer, End> {
        tokio::time::timeout(ANSWER, self.recv())
            .await
            .unwrap_or_else(|_| Err(lost()))
    }

    /// The next message from the relay.
    async fn recv(&mut self) -> Result<Answer, End> {
        loop {
            match self.ws.next().await {
                Some(Ok(Message::Binary(bytes))) => {
                    return wire::decode(&bytes).map_err(|_| lost());
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => {}
                _ => return Err(lost()),
            }
        }
    }
}
