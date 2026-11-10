//! Pausing and resuming sync (RFC 0009, "Sync and status"), for an app
//! that wants to stay offline. A pause is kept under the database's lock,
//! where the sync thread checks it before it sends and before it records
//! anything, so from the moment `pause` returns the SDK sends the relay
//! nothing; the thread then drops the connection and waits for `resume`.
//! A session stays halted until the thread has dropped it, even when the
//! app resumes before the thread noticed the pause.
//! A pause lasts while the database is open: a reopened database syncs.

use super::{Connection, SyncReport};

impl SyncReport<'_> {
    /// Stops syncing: closes the connection, and reports
    /// [`Connection::Paused`] until [`SyncReport::resume`]. Writes stay
    /// unconfirmed meanwhile. A database opened without a relay has
    /// nothing to pause.
    pub async fn pause(&self) {
        let mut inner = self.db.lock();
        let Some(sync) = inner.sync.as_mut() else {
            return;
        };
        sync.halted = true;
        sync.pause.send_replace(true);
        sync.connection = Connection::Paused;
        sync.live.clear();
        inner.status.notify();
    }

    /// Syncs again after [`SyncReport::pause`]: connects at once, with
    /// the backoff started over, and pushes what was written meanwhile.
    pub async fn resume(&self) {
        let mut inner = self.db.lock();
        let Some(sync) = inner.sync.as_mut() else {
            return;
        };
        if !sync.paused() {
            return;
        }
        sync.pause.send_replace(false);
        sync.connection = Connection::Connecting;
        inner.status.notify();
    }
}
