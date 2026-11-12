//! Syncing with one relay (RFC 0009, "Sync and status"; RFC 0007, "The
//! protocol" and "Island mode"). A database opened with
//! [`OpenOptions::relay`](crate::OpenOptions::relay) syncs on a thread of
//! its own, with a tokio runtime of its own, so the app needs none; in the
//! browser, on the event loop of the page or worker that opened it, over
//! the browser's WebSocket (`transport`; NOC-175). Every
//! call of the app stays on the device; syncing only folds what it pulls
//! and records what the relay answered, and reports it in
//! [`Db::sync`].
//!
//! `run` keeps one connection up and reconnects with backoff and jitter;
//! `session` serves one connection; `apply` changes the database for it,
//! under the database's lock; `wire` reads and writes RFC 0007's messages.
//!
//! A space this member was added to is synced and folded, and
//! [`Spaces::get`](crate::Spaces::get) gives the handle that writes into
//! it.
//!
//! `status` follows the report and the confirmation of one write; `pause`
//! stops and restarts syncing; `again` pushes a refused change once more
//! when the member's own fold newly accepts it (RFC 0008, M19); `large`
//! carries a change too large for one message through the relay's upload
//! and download URLs (RFC 0007, "Large changes"), with the small HTTP/1.1
//! client of `http`, or in the browser with `fetch()` (`http_browser`).
//!
//! **What this does not do yet.** The per-space `status` of RFC 0009 is
//! not built: the relay's refusals are recorded per change in the log, by
//! the same code `SyncStatus::error` would show (NOC-188), and
//! [`SyncStatus`] counts them. A log written before NOC-188 may hold a
//! relay's own text there, so the API that first shows a refusal's code
//! must read it through `wire::mapped_code`. The relay is reached over plain
//! WebSocket over TLS (`wss://`) or, on a local network, without it
//! (`ws://`); its upload and download URLs likewise over `https://` or
//! `http://`. TLS verifies the relay against the platform's trust store
//! (`tls`), or in the browser against the browser's.

pub(crate) mod acknowledge;
mod again;
mod apply;
mod finalize;
#[cfg(not(target_arch = "wasm32"))]
mod http;
#[cfg(target_arch = "wasm32")]
#[path = "http_browser.rs"]
mod http;
mod large;
mod merge;
mod pause;
mod person_keys;
#[cfg(test)]
mod person_keys_tests;
mod run;
mod session;
mod status;
pub(crate) mod time;
#[cfg(test)]
mod time_tests;
#[cfg(not(target_arch = "wasm32"))]
mod tls;
mod transport;
mod wire;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::ids::{ChangeId as CoreChangeId, SpaceId as CoreSpaceId};
use tokio::sync::watch;

use crate::db::{Db, Inner};

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use finalize::set_finalize_after;
#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) use merge::set_merge_delay;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use person_keys::put as put_person_keys;
pub(crate) use run::start;
pub(crate) use status::{Notify, follow};
pub(crate) use wire::Record;

/// The state of the relay connection (RFC 0009, "Sync and status").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Connection {
    /// Connected and authenticated.
    Connected,
    /// Opening a connection.
    Connecting,
    /// No relay reachable; the SDK retries with backoff.
    Offline,
    /// Syncing was paused by the app.
    Paused,
    /// The database was opened without a relay.
    None,
}

/// What [`SyncReport::current`] reports: how far this member's copy is
/// synced with the relay.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SyncStatus {
    /// The state of the relay connection.
    pub connection: Connection,
    /// This member's changes the relay has neither confirmed nor refused.
    pub unconfirmed: u64,
    /// This member's changes the relay refused. They stay folded on this
    /// member (RFC 0002, D12).
    pub refused: u64,
    /// Whether the relay was restored and the SDK is pushing again what it
    /// lost (RFC 0007, "Restores and the relay epoch").
    pub epoch_changed: bool,
    /// The last error that stops syncing, by its RFC 0007 code, such as
    /// `unsupported_version`; never content. A code the relay sent that
    /// RFC 0007 does not list shows as `unknown_error`, so the relay's own
    /// text never reaches the app (NOC-146). Once the relay answered
    /// `not_member` or `removed_member` for a space, that code stays here,
    /// since the space is no longer synced, while no error of the
    /// connection replaces it and until the relay says, with `added`, that
    /// the member was added again to every space it stopped.
    pub error: Option<String>,
}

/// The sync report of a database, from [`Db::sync`].
#[derive(Debug)]
pub struct SyncReport<'a> {
    db: &'a Db,
}

impl Db {
    /// The report of the relay connection and of what syncing found.
    pub fn sync(&self) -> SyncReport<'_> {
        SyncReport { db: self }
    }
}

impl SyncReport<'_> {
    /// The report as it is now.
    pub fn current(&self) -> SyncStatus {
        status(&self.db.lock())
    }
}

/// The sync status of the database `inner`.
fn status(inner: &Inner) -> SyncStatus {
    let (unconfirmed, refused) = apply::own_counts(inner);
    let sync = inner.sync.as_ref();
    SyncStatus {
        connection: sync.map_or(Connection::None, |s| s.connection),
        unconfirmed,
        refused,
        epoch_changed: sync.is_some_and(|s| s.epoch_changed),
        error: sync.and_then(|s| s.error.clone().or_else(|| s.removed.clone())),
    }
}

/// The longest an admin's SDK waits before it writes a merge rotation
/// (RFC 0008, "Two removals that rotate the key at once", step 5).
const MERGE_DELAY: std::time::Duration = std::time::Duration::from_secs(60);

/// How long a device of the performer's account holds a provisional shred
/// before it finalizes it: 7 days (RFC 0008, "A provisional erasure in a
/// space with one account").
const FINALIZE_AFTER: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);

/// How long after a `too_early` refusal the SDK pushes the finalize again:
/// an hour, since the device cannot read the relay's clock (pm, NOC-234).
const TOO_EARLY_RETRY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// What a database that syncs keeps of its relay.
pub(crate) struct SyncState {
    pub(crate) connection: Connection,
    pub(crate) epoch_changed: bool,
    pub(crate) error: Option<String>,
    /// The relay epoch the positions in the log belong to.
    pub(crate) epoch: Option<[u8; 16]>,
    /// The cursor of each space: the highest relay position folded, with
    /// every position below it.
    pub(crate) cursors: BTreeMap<CoreSpaceId, u64>,
    /// The spaces the relay said this member may no longer read, until it
    /// says the member was added to them again.
    pub(crate) stopped: BTreeSet<CoreSpaceId>,
    /// The stopped spaces whose unconfirmed changes are pushed once more:
    /// the relay confirms those it holds as duplicates, such as a leave
    /// whose answer was lost, and refuses the rest (RFC 0007, "Push": a
    /// push is idempotent; NOC-276). A space leaves this set once that
    /// push is done.
    pub(crate) flush: BTreeSet<CoreSpaceId>,
    /// The code the relay last stopped a space with, `not_member` or
    /// `removed_member`.
    pub(crate) removed: Option<String>,
    /// The spaces whose subscription in this connection caught up with
    /// the relay's head. From then on the relay answers a removal from
    /// the space with `removed_member`; before, with `not_member` (RFC
    /// 0007, "Subscribe").
    pub(crate) live: BTreeSet<CoreSpaceId>,
    /// The relay head of each space, the highest position the relay told
    /// the member of in this epoch.
    pub(crate) heads: BTreeMap<CoreSpaceId, u64>,
    /// The spaces whose cursor reached the relay's head once in this
    /// epoch, since the database opened: their initial sync is complete.
    pub(crate) pulled: BTreeSet<CoreSpaceId>,
    /// How many changes syncing folded that the log did not hold.
    pub(crate) folds: u64,
    /// The changes the relay refused, per space, each with whether the
    /// member's fold rejected it since: one folded again is pushed once
    /// more (RFC 0008, M19).
    pub(crate) refused: BTreeMap<CoreSpaceId, BTreeMap<CoreChangeId, bool>>,
    /// The person-key records this member wrote that wait to be put, per
    /// space (`person_keys`).
    pub(crate) puts: BTreeMap<CoreSpaceId, Vec<Record>>,
    /// When this admin writes the merge rotation of each space whose
    /// current generation is exposed, unless the exposure ends first
    /// (`merge`; RFC 0008, "Two removals that rotate the key at once",
    /// step 5).
    pub(crate) merges: BTreeMap<CoreSpaceId, time::Instant>,
    /// The longest delay before a merge rotation: a minute, which the
    /// tests change (`set_merge_delay`).
    pub(crate) merge_delay: std::time::Duration,
    /// How long this device holds a provisional shred before an admin of
    /// the performer's account finalizes it: 7 days, which the tests
    /// change (`finalize`; RFC 0008, "A provisional erasure in a space
    /// with one account").
    pub(crate) finalize_after: std::time::Duration,
    /// How long after a `too_early` refusal the finalize is pushed again:
    /// an hour, which the tests change (pm, NOC-234).
    pub(crate) too_early_retry: std::time::Duration,
    /// When each space whose finalize the relay refused as `too_early` is
    /// pushed again (`finalize`).
    pub(crate) retries: BTreeMap<CoreSpaceId, time::Instant>,
    /// Tells the sync thread that the database wrote; dropped with the
    /// database, which ends the thread.
    wake: watch::Sender<()>,
    /// Whether the app paused syncing, which the sync thread watches.
    pub(crate) pause: watch::Sender<bool>,
    /// Set by a pause, cleared only by the sync thread once it dropped the
    /// connection and the app resumed: a session open when the app paused
    /// sends and records nothing more, even after an immediate resume.
    pub(crate) halted: bool,
}

impl SyncState {
    fn new(wake: watch::Sender<()>, pause: watch::Sender<bool>) -> SyncState {
        SyncState {
            connection: Connection::Connecting,
            epoch_changed: false,
            error: None,
            epoch: None,
            cursors: BTreeMap::new(),
            stopped: BTreeSet::new(),
            flush: BTreeSet::new(),
            removed: None,
            live: BTreeSet::new(),
            heads: BTreeMap::new(),
            pulled: BTreeSet::new(),
            folds: 0,
            refused: BTreeMap::new(),
            puts: BTreeMap::new(),
            merges: BTreeMap::new(),
            merge_delay: MERGE_DELAY,
            finalize_after: FINALIZE_AFTER,
            too_early_retry: TOO_EARLY_RETRY,
            retries: BTreeMap::new(),
            wake,
            pause,
            halted: false,
        }
    }

    /// Whether the app paused syncing.
    pub(crate) fn paused(&self) -> bool {
        *self.pause.borrow()
    }
}

/// Whether the relay said this member may no longer read `space` (RFC
/// 0007, `not_member` and `removed_member`): the member was removed, even
/// where its copy has not folded the removal, which the relay does not
/// serve it.
pub(crate) fn stopped(inner: &Inner, space: CoreSpaceId) -> bool {
    inner
        .sync
        .as_ref()
        .is_some_and(|sync| sync.stopped.contains(&space))
}

/// How far `space` is synced (RFC 0009, "Sync and status"): whether its
/// initial sync is complete, and how many relay positions the member heard
/// of but has not folded. Without a relay, `false` and 0.
pub(crate) fn progress(inner: &Inner, space: CoreSpaceId) -> (bool, u64) {
    let Some(sync) = inner.sync.as_ref() else {
        return (false, 0);
    };
    let cursor = sync.cursors.get(&space).copied().unwrap_or(0);
    let head = sync.heads.get(&space).copied().unwrap_or(0);
    (sync.pulled.contains(&space), head.saturating_sub(cursor))
}

/// The code of a relay's refusal as the SDK shows it: one RFC 0007 lists,
/// or `unknown_error` (NOC-188).
pub(crate) fn mapped_code(code: &str) -> String {
    wire::mapped_code(code)
}

/// Tells the sync thread, if there is one, and the followers of the sync
/// status that the database wrote.
pub(crate) fn wake(inner: &Inner) {
    if let Some(sync) = &inner.sync {
        sync.wake.send_replace(());
    }
    inner.status.notify();
}

#[cfg(all(test, not(target_arch = "wasm32")))]
impl Inner {
    /// How many changes syncing folded that the log did not hold.
    pub(crate) fn sync_folds(&self) -> u64 {
        self.sync.as_ref().map_or(0, |s| s.folds)
    }

    /// Whether the subscription to `space` caught up in this connection.
    pub(crate) fn sync_live(&self, space: CoreSpaceId) -> bool {
        self.sync.as_ref().is_some_and(|s| s.live.contains(&space))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod tests;
