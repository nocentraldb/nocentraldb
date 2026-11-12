//! One connection to the relay (RFC 0007, "Opening a connection" and
//! "Island mode"): authenticate, then for every space pull from its
//! cursor to the head, push what the relay has not confirmed in local
//! order, and subscribe from the new cursor; then fold live entries as
//! they arrive and push each new write.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use ncdb_core::ids::SpaceId;
use tokio::sync::watch;

use super::time::{self, Instant};
use super::transport::{self, Transport};
use super::wire::{self, Answer};
use super::{Connection, apply, finalize, large, merge};
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

/// Waits until `deadline`, or forever without one.
async fn until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

/// What became of an upload.
enum Uploaded {
    /// The frame is uploaded under this id; the push names it.
    Id(Vec<u8>),
    /// The relay refused the frame as too large; it is recorded as refused.
    Refused,
    /// The relay answered with another error, handled as an answer to the
    /// push, which stops.
    Stopped,
}

/// How a session ended.
pub(super) enum End {
    /// The database was dropped: syncing ends.
    Dropped,
    /// The app paused syncing: the connection is dropped until it resumes.
    Paused,
    /// The connection was lost or the relay ended it; the member
    /// reconnects. `progress` says whether the session synced every space
    /// once, so that the backoff starts over; `retry_after` is how long the
    /// relay asked the member to wait (RFC 0007, `rate_limited`).
    Lost {
        progress: bool,
        retry_after: Option<Duration>,
    },
}

struct Session<'a, T: Transport> {
    ws: T,
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
    /// For each space whose log names a person key the member does not
    /// hold, when to fetch the person keys again and the wait after that
    /// (`person_keys::missing`).
    refetch: BTreeMap<SpaceId, (Instant, Duration)>,
}

/// How long after a fetch that left a named person key missing the
/// session fetches again, the first time; each later wait doubles, up to
/// [`IDLE`]. A record follows its entry, which its writer pushed first,
/// so it is usually there a moment later (RFC 0007, "Keys and the
/// person-key store").
const REFETCH: Duration = Duration::from_millis(250);

/// The connection ended. An error code that stops syncing is reported by
/// [`Session::failed`]; whether the session made progress is filled in
/// when it ends.
fn lost() -> End {
    End::Lost {
        progress: false,
        retry_after: None,
    }
}

/// Runs `f` on the database, unless it was dropped or the app paused
/// syncing, and tells the followers of the sync status.
fn with<R>(db: &Weak<Mutex<Inner>>, f: impl FnOnce(&mut Inner) -> R) -> Result<R, End> {
    let inner: Arc<Mutex<Inner>> = db.upgrade().ok_or(End::Dropped)?;
    let mut guard = lock(&inner);
    running(&guard)?;
    let result = f(&mut guard);
    guard.status.notify();
    Ok(result)
}

/// Whether the session may go on: the app has not paused syncing since
/// it opened.
fn running(inner: &Inner) -> Result<(), End> {
    match inner.sync.as_ref().is_some_and(|sync| sync.halted) {
        true => Err(End::Paused),
        false => Ok(()),
    }
}

/// One connection to the relay at `url`, served until it ends.
pub(super) async fn session(
    db: &Weak<Mutex<Inner>>,
    url: &str,
    wake: &mut watch::Receiver<()>,
) -> End {
    let Ok(Some(ws)) = time::timeout(CONNECT, transport::connect(url)).await else {
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
        refetch: BTreeMap::new(),
    };
    if let Err(end) = session.authenticate().await {
        return end;
    }
    match session.serve(wake).await.err().unwrap_or(lost()) {
        End::Lost { retry_after, .. } => End::Lost {
            progress: session.progress,
            retry_after,
        },
        end @ (End::Dropped | End::Paused) => end,
    }
}

impl<T: Transport> Session<'_, T> {
    /// hello, challenge, auth, welcome.
    async fn authenticate(&mut self) -> Result<(), End> {
        self.send(wire::hello()).await?;
        let (relay_id, nonce) = match self.answer_in_time().await? {
            Answer::Challenge { relay_id, nonce } => (relay_id, nonce),
            Answer::Error { code, .. } => return Err(self.failed(code)),
            _ => return Err(lost()),
        };
        let digest = wire::auth_digest(&relay_id, &nonce);
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
            for state in inner.spaces.values_mut() {
                state.person_keys_fetched = false;
            }
            Ok(())
        })?
        .map_err(|_: ncdb_core::view::Error| lost())
    }

    /// Records an error the relay ended the connection with.
    fn failed(&self, code: String) -> End {
        let _ = with(self.db, |inner| {
            if let Some(sync) = inner.sync.as_mut() {
                sync.error = Some(wire::mapped_code(&code));
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
                    // A pending push of the space waits for the pull, but
                    // is not dropped.
                    if self.pull(space).await? && push {
                        self.push(space).await?;
                    }
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
            // Merge rotations whose delay passed are written now; the
            // write wakes this loop, which pushes them.
            let due = with(self.db, |inner| merge::due(inner, Instant::now()))?;
            for space in due {
                with(self.db, |inner| merge::write(inner, space))?;
            }
            // Finalizes whose 7 days passed on this device are written now,
            // and finalizes the relay refused as too early are pushed again
            // (RFC 0008, "A provisional erasure in a space with one
            // account").
            let due = with(self.db, |inner| {
                finalize::due(inner, crate::clock::now_millis())
            })?;
            for (space, shred) in due {
                with(self.db, |inner| finalize::write(inner, space, shred))?;
            }
            let again = with(self.db, |inner| {
                finalize::retries_due(inner, Instant::now())
            })?;
            for space in again {
                self.push(space).await?;
            }
            // A person key the log names but the member does not hold, as
            // after an entry that announced a new person, is fetched again
            // (RFC 0009, "How a personal field reads": the object reads
            // again once the key arrives).
            let missing = with(self.db, |inner| {
                self.subscribed
                    .iter()
                    .copied()
                    .filter(|space| super::person_keys::missing(inner, *space))
                    .collect::<BTreeSet<_>>()
            })?;
            self.refetch.retain(|space, _| missing.contains(space));
            for space in missing {
                let now = Instant::now();
                match self.refetch.get(&space).copied() {
                    Some((at, _)) if at > now => {}
                    previous => {
                        self.fetch_person_keys(space).await?;
                        let wait = previous.map_or(REFETCH, |(_, wait)| (wait * 2).min(IDLE));
                        self.refetch.insert(space, (Instant::now() + wait, wait));
                    }
                }
            }
            let refetch = self.refetch.values().map(|(at, _)| *at).min();
            let deadline = with(self.db, |inner| {
                let finalizes = finalize::next(inner, crate::clock::now_millis(), Instant::now());
                [merge::next(inner), finalizes, refetch]
                    .into_iter()
                    .flatten()
                    .min()
            })?;
            tokio::select! {
                answer = self.recv() => self.live(answer?)?,
                changed = wake.changed() => match changed {
                    Ok(()) => push = true,
                    Err(_) => return Err(End::Dropped),
                },
                () = until(deadline) => {}
                () = time::sleep(IDLE) => {
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
        self.fetch_person_keys(space).await?;
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
                    let (entries, complete) = large::fetch(entries).await;
                    self.entries(space, entries)?;
                    // A download that failed: the cursor stays before its
                    // entry, and the session backs off and pulls again.
                    if !complete {
                        return Err(lost());
                    }
                    let cursor = with(self.db, |inner| {
                        apply::heard(inner, space, head);
                        apply::cursor(inner, space)
                    })?;
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
                Answer::Error { code, .. } if code == "unknown_space" => {
                    with(self.db, |inner| apply::heard(inner, space, 0))?;
                    return Ok(true);
                }
                answer => return self.answer(space, answer).map(|()| false),
            }
        }
    }

    /// Pushes every change of `space` the relay has not confirmed, in
    /// local order, in pushes that each fit one message.
    async fn push(&mut self, space: SpaceId) -> Result<(), End> {
        // A space whose finalize the relay found too early waits until its
        // push again is due (`finalize`).
        if with(self.db, |inner| finalize::waiting(inner, space))? {
            return Ok(());
        }
        let unconfirmed =
            with(self.db, |inner| apply::unconfirmed(inner, space))?.map_err(|_| lost())?;
        let (ids, frames): (Vec<_>, Vec<_>) = unconfirmed.into_iter().unzip();
        let mut ids = ids.into_iter();
        for batch in wire::batches(frames) {
            let batch_ids: Vec<_> = ids.by_ref().take(batch.len()).collect();
            let req = self.req();
            match &batch[..] {
                [frame] if !wire::fits_a_push(frame) => {
                    let id = match self.upload(space, batch_ids[0], frame).await? {
                        Uploaded::Id(id) => id,
                        Uploaded::Refused => continue,
                        Uploaded::Stopped => return Ok(()),
                    };
                    self.send(wire::push_upload(req, space, &id)).await?;
                }
                _ => self.send(wire::push(req, space, &batch)).await?,
            }
            match self.ask(req).await? {
                Answer::Pushed { results, .. } => {
                    let stopped = with(self.db, |inner| {
                        apply::pushed(inner, space, &batch_ids, results)
                    })?
                    .map_err(|_| lost())?;
                    match stopped {
                        Some(code) if code == apply::TOO_EARLY => return Ok(()),
                        Some(_) => return Err(lost()),
                        None => {}
                    }
                }
                answer => return self.answer(space, answer),
            }
        }
        self.put_person_keys(space).await
    }

    /// Uploads `frame`, too large for one push, to an upload URL the relay
    /// hands out (RFC 0007, "Large changes"). A frame the relay refuses as
    /// `too_large` is recorded as refused, as a push would; any other error
    /// is handled as an answer to the push, which then stops. An upload that
    /// fails ends the connection, and the change is pushed again on the next
    /// one.
    async fn upload(
        &mut self,
        space: SpaceId,
        change: ncdb_core::ids::ChangeId,
        frame: &[u8],
    ) -> Result<Uploaded, End> {
        let req = self.req();
        self.send(wire::upload(req, space, frame.len())).await?;
        match self.ask(req).await? {
            Answer::Upload { id, url, .. } => {
                large::upload(&url, frame).await.map_err(|_| lost())?;
                Ok(Uploaded::Id(id))
            }
            Answer::Error { code, .. } if code == "too_large" => {
                let refused = vec![wire::Pushed::Rejected(code)];
                with(self.db, |inner| {
                    apply::pushed(inner, space, &[change], refused)
                })?
                .map_err(|_| lost())?;
                Ok(Uploaded::Refused)
            }
            answer => self.answer(space, answer).map(|()| Uploaded::Stopped),
        }
    }

    /// Puts the person-key records of `space` this member wrote, once the
    /// relay confirmed the changes that announce them (RFC 0007,
    /// `put-person-keys`; `person_keys::ready`); the others wait for a
    /// later push. A record the relay refuses with `bad_record` is
    /// dropped, the whole request with it, since the relay stores none of
    /// them; any other answer, or a connection that ends first, keeps them
    /// for the next connection.
    async fn put_person_keys(&mut self, space: SpaceId) -> Result<(), End> {
        let records = with(self.db, |inner| {
            let waiting = inner
                .sync
                .as_mut()
                .and_then(|sync| sync.puts.remove(&space))
                .unwrap_or_default();
            let (send, keep) = super::person_keys::ready(inner, space, waiting);
            if let Some(sync) = inner.sync.as_mut()
                && !keep.is_empty()
            {
                sync.puts.entry(space).or_default().extend(keep);
            }
            send
        })?;
        if records.is_empty() {
            return Ok(());
        }
        let keep = |records: Vec<wire::Record>| {
            with(self.db, |inner| {
                if let Some(sync) = inner.sync.as_mut() {
                    sync.puts.entry(space).or_default().extend(records);
                }
            })
        };
        let req = self.req();
        let answer = match self.send(wire::put_person_keys(req, space, &records)).await {
            Ok(()) => self.ask(req).await,
            Err(end) => Err(end),
        };
        match answer {
            Ok(Answer::Other { .. }) => Ok(()),
            Ok(Answer::Error { code, .. }) if code == "bad_record" => Ok(()),
            Ok(answer) => {
                keep(records)?;
                self.answer(space, answer)
            }
            Err(end) => {
                // Kept for the next connection; `with` fails only once the
                // database is gone, when nothing is left to keep them for.
                let _ = keep(records);
                Err(end)
            }
        }
    }

    /// Fetches the person-key records of `space`, when its log names a
    /// person key, holds each person key whose record holds
    /// (`person_keys::accept`), and keeps the accounts of the writers whose
    /// signed records failed, which the space status names.
    async fn fetch_person_keys(&mut self, space: SpaceId) -> Result<(), End> {
        let named = with(self.db, |inner| {
            inner
                .spaces
                .get(&space)
                .is_some_and(|state| state.checks.resolved().person_keys().next().is_some())
        })?;
        if named {
            let req = self.req();
            self.send(wire::person_keys(req, space)).await?;
            match self.ask(req).await? {
                Answer::PersonKeys { records, .. } => {
                    with(self.db, |inner| {
                        if let Some(state) = inner.spaces.get_mut(&space) {
                            let mut bad = std::collections::BTreeSet::new();
                            for record in &records {
                                let outcome = super::person_keys::accept(state, space, record);
                                if let super::person_keys::Outcome::Bad(account) = outcome {
                                    bad.insert(account);
                                }
                            }
                            state.bad_person_key_records = bad;
                        }
                    })?;
                }
                answer => return self.answer(space, answer),
            }
        }
        with(self.db, |inner| {
            let crate::db::Inner { view, spaces, .. } = inner;
            let Some(state) = spaces.get_mut(&space) else {
                return Ok(());
            };
            state.person_keys_fetched = true;
            // The fetched keys open the space's personal fields.
            crate::personal::refresh(view, space, state)
        })?
        .map_err(|_| lost())
    }

    /// Handles an answer about `space` that is not the one asked for: an
    /// error, or entries of a subscription.
    fn answer(&mut self, space: SpaceId, answer: Answer) -> Result<(), End> {
        match answer {
            Answer::Entries { entries, head, .. } => {
                // An entry that comes as a download link waits for a pull,
                // which fetches it: the space has a gap from there.
                let (entries, complete) = large::inline(entries);
                self.entries(space, entries)?;
                let cursor = with(self.db, |inner| {
                    apply::heard(inner, space, head);
                    apply::cursor(inner, space)
                })?;
                if cursor < head || !complete {
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

    /// Sends `message`, unless the app paused syncing meanwhile.
    async fn send(&mut self, message: Vec<u8>) -> Result<(), End> {
        let inner = self.db.upgrade().ok_or(End::Dropped)?;
        running(&lock(&inner))?;
        drop(inner);
        self.ws.send(message).await.map_err(|_| lost())
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
        time::timeout(ANSWER, answer)
            .await
            .unwrap_or_else(|_| Err(lost()))
    }

    /// The next message, within [`ANSWER`].
    async fn answer_in_time(&mut self) -> Result<Answer, End> {
        time::timeout(ANSWER, self.recv())
            .await
            .unwrap_or_else(|_| Err(lost()))
    }

    /// The next message from the relay.
    async fn recv(&mut self) -> Result<Answer, End> {
        match self.ws.recv().await {
            Some(bytes) => wire::decode(&bytes).map_err(|_| lost()),
            None => Err(lost()),
        }
    }
}
