//! The sync thread: one connection at a time, reconnected with
//! exponential backoff and jitter while the relay is unreachable or in
//! trouble (RFC 0007, "Liveness" and "Island mode"). The backoff starts
//! over only after a session synced every space once, it waits at least
//! the retry-after of `rate_limited`, and a local write does not cut it
//! short. The thread ends when the database is dropped.

use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use tokio::sync::watch;

use super::session::{End, session};
use super::{Connection, SyncState};
use crate::db::{Inner, lock};

/// The first wait before a reconnect.
const FIRST: Duration = Duration::from_millis(100);

/// The longest wait between two attempts: a few minutes (RFC 0007,
/// "Island mode").
const LONGEST: Duration = Duration::from_secs(120);

/// Starts syncing the database `inner` with the relay at `url`.
pub(crate) fn start(inner: &Arc<Mutex<Inner>>, url: String) {
    let (wake, receiver) = watch::channel(());
    lock(inner).sync = Some(SyncState::new(wake));
    let db = Arc::downgrade(inner);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let started = runtime.map(|runtime| {
        std::thread::Builder::new()
            .name("nocentraldb-sync".into())
            .spawn(move || runtime.block_on(run(db, url, receiver)))
    });
    if !matches!(started, Ok(Ok(_))) {
        set(inner, Connection::Offline, Some("unavailable".into()));
    }
}

fn set(inner: &Mutex<Inner>, connection: Connection, error: Option<String>) {
    if let Some(sync) = lock(inner).sync.as_mut() {
        sync.connection = connection;
        if error.is_some() {
            sync.error = error;
        }
    }
}

async fn run(db: Weak<Mutex<Inner>>, url: String, mut wake: watch::Receiver<()>) {
    let mut delay = FIRST;
    loop {
        let Some(inner) = db.upgrade() else { return };
        set(&inner, Connection::Connecting, None);
        drop(inner);
        let (progress, retry_after) = match session(&db, &url, &mut wake).await {
            End::Dropped => return,
            End::Lost {
                progress,
                retry_after,
            } => (progress, retry_after),
        };
        let Some(inner) = db.upgrade() else { return };
        set(&inner, Connection::Offline, None);
        drop(inner);
        if progress {
            delay = FIRST;
        }
        let wait = jitter(delay).max(retry_after.unwrap_or_default());
        if !sleep(wait, &mut wake).await {
            return;
        }
        delay = (delay * 2).min(LONGEST);
    }
}

/// Sleeps for `wait`; a local write does not cut it short. Returns
/// `false` when the database was dropped meanwhile.
async fn sleep(wait: Duration, wake: &mut watch::Receiver<()>) -> bool {
    let sleep = tokio::time::sleep(wait);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            () = &mut sleep => return true,
            changed = wake.changed() => if changed.is_err() { return false },
        }
    }
}

/// Half to all of `delay`, drawn at random, so that many members do not
/// reconnect in the same second.
fn jitter(delay: Duration) -> Duration {
    let mut byte = [0u8; 1];
    let fraction = match getrandom::fill(&mut byte) {
        Ok(()) => 0.5 + f64::from(byte[0]) / 510.0,
        Err(_) => 1.0,
    };
    delay.mul_f64(fraction)
}
