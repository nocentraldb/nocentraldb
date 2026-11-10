//! Syncing with one relay (RFC 0009, "Sync and status"; RFC 0007, "The
//! protocol" and "Island mode"). A database opened with
//! [`OpenOptions::relay`](crate::OpenOptions::relay) syncs on a thread of
//! its own, with a tokio runtime of its own, so the app needs none. Every
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
//! **What this does not do yet.** `db.sync().subscribe()`, `pause()`,
//! `resume()`, `is_confirmed()`, `confirmed()` and the per-space `status`
//! of RFC 0009 are not built: the relay's refusals are recorded per change
//! in the log, and [`SyncStatus`] counts them. The relay is reached over plain
//! WebSocket (`ws://`) only; TLS is NOC-121. A large change, which the
//! relay hands out as a download link, is not read: the page that holds
//! it ends the session, which backs off and meets the same page again,
//! until NOC-121 reads the link.

mod apply;
mod run;
mod session;
mod wire;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::ids::SpaceId as CoreSpaceId;
use tokio::sync::watch;

use crate::db::{Db, Inner};

pub(crate) use run::start;

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
    /// `unsupported_version`; never content.
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
        let inner = self.db.lock();
        let (unconfirmed, refused) = apply::own_counts(&inner);
        let sync = inner.sync.as_ref();
        SyncStatus {
            connection: sync.map_or(Connection::None, |s| s.connection),
            unconfirmed,
            refused,
            epoch_changed: sync.is_some_and(|s| s.epoch_changed),
            error: sync.and_then(|s| s.error.clone()),
        }
    }
}

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
    /// The spaces the relay said this member may no longer read.
    pub(crate) stopped: BTreeSet<CoreSpaceId>,
    /// How many changes syncing folded that the log did not hold.
    pub(crate) folds: u64,
    /// Tells the sync thread that the database wrote; dropped with the
    /// database, which ends the thread.
    wake: watch::Sender<()>,
}

impl SyncState {
    fn new(wake: watch::Sender<()>) -> SyncState {
        SyncState {
            connection: Connection::Connecting,
            epoch_changed: false,
            error: None,
            epoch: None,
            cursors: BTreeMap::new(),
            stopped: BTreeSet::new(),
            folds: 0,
            wake,
        }
    }
}

/// Tells the sync thread, if there is one, that the database wrote.
pub(crate) fn wake(inner: &Inner) {
    if let Some(sync) = &inner.sync {
        sync.wake.send_replace(());
    }
}

#[cfg(test)]
impl Inner {
    /// How many changes syncing folded that the log did not hold.
    pub(crate) fn sync_folds(&self) -> u64 {
        self.sync.as_ref().map_or(0, |s| s.folds)
    }
}

#[cfg(test)]
mod tests;
