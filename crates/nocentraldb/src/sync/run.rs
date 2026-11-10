//! The sync thread: one connection at a time, reconnected with
//! exponential backoff and jitter while the relay is unreachable or in
//! trouble (RFC 0007, "Liveness" and "Island mode"). The backoff starts
//! over only after a session synced every space once, it waits at least
//! the retry-after of `rate_limited`, and a local write does not cut it
//! short. A pause drops the connection, or the wait, and holds the thread
//! until the app resumes; resuming connects at once. The thread ends when
//! the database is dropped.

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
    let (pause, paused) = watch::channel(false);
    lock(inner).sync = Some(SyncState::new(wake, pause));
    let db = Arc::downgrade(inner);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let started = runtime.map(|runtime| {
        std::thread::Builder::new()
            .name("nocentraldb-sync".into())
            .spawn(move || runtime.block_on(run(db, url, receiver, paused)))
    });
    if !matches!(started, Ok(Ok(_))) {
        set(inner, Connection::Offline, Some("unavailable".into()));
    }
}

/// Records the state of the connection; a paused database stays
/// `Paused`.
fn set(inner: &Mutex<Inner>, connection: Connection, error: Option<String>) {
    let mut inner = lock(inner);
    if let Some(sync) = inner.sync.as_mut() {
        if !sync.paused() {
            sync.connection = connection;
        }
        // Connecting, offline or paused, no subscription is live.
        sync.live.clear();
        if error.is_some() {
            sync.error = error;
        }
    }
    inner.status.notify();
}

async fn run(
    db: Weak<Mutex<Inner>>,
    url: String,
    mut wake: watch::Receiver<()>,
    mut pause: watch::Receiver<bool>,
) {
    let mut delay = FIRST;
    loop {
        if !resumed(&db, &mut pause).await {
            return;
        }
        let Some(inner) = db.upgrade() else { return };
        set(&inner, Connection::Connecting, None);
        drop(inner);
        let end = tokio::select! {
            end = session(&db, &url, &mut wake) => end,
            () = halted(&db, &mut pause) => End::Paused,
        };
        let (progress, retry_after) = match end {
            End::Dropped => return,
            End::Paused => {
                delay = FIRST;
                continue;
            }
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
        match sleep(wait, &db, &mut wake, &mut pause).await {
            Slept::Dropped => return,
            Slept::Paused => {
                delay = FIRST;
                continue;
            }
            Slept::Elapsed => {}
        }
        delay = (delay * 2).min(LONGEST);
    }
}

/// Waits while the app has paused syncing, then lifts the halt of the
/// session it dropped, so the next one may sync. Returns `false` when the
/// database was dropped meanwhile.
async fn resumed(db: &Weak<Mutex<Inner>>, pause: &mut watch::Receiver<bool>) -> bool {
    loop {
        pause.borrow_and_update();
        {
            let Some(inner) = db.upgrade() else {
                return false;
            };
            let mut inner = lock(&inner);
            let Some(sync) = inner.sync.as_mut() else {
                return false;
            };
            if !sync.paused() {
                sync.halted = false;
                return true;
            }
        }
        if pause.changed().await.is_err() {
            return false;
        }
    }
}

/// Resolves when the app has paused syncing since the session opened,
/// even if it resumed since; never, once the database was dropped, which
/// the session notices on its own.
async fn halted(db: &Weak<Mutex<Inner>>, pause: &mut watch::Receiver<bool>) {
    loop {
        pause.borrow_and_update();
        let halted = db
            .upgrade()
            .is_some_and(|inner| lock(&inner).sync.as_ref().is_some_and(|s| s.halted));
        if halted {
            return;
        }
        if pause.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// How a wait between two attempts ended.
enum Slept {
    Elapsed,
    Paused,
    Dropped,
}

/// Sleeps for `wait`; a local write does not cut it short, a pause does.
async fn sleep(
    wait: Duration,
    db: &Weak<Mutex<Inner>>,
    wake: &mut watch::Receiver<()>,
    pause: &mut watch::Receiver<bool>,
) -> Slept {
    let sleep = tokio::time::sleep(wait);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            () = &mut sleep => return Slept::Elapsed,
            changed = wake.changed() => if changed.is_err() { return Slept::Dropped },
            () = halted(db, pause) => return Slept::Paused,
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
