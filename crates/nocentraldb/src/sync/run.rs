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
    {
        let mut inner = lock(inner);
        inner.sync = Some(SyncState::new(wake, pause));
        // A log that cannot be read leaves nothing to push again; pushing
        // reads it again and ends the session.
        let _ = super::again::load(&mut inner);
    }
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
        if !resumed(&mut pause).await {
            return;
        }
        let Some(inner) = db.upgrade() else { return };
        set(&inner, Connection::Connecting, None);
        drop(inner);
        let end = tokio::select! {
            end = session(&db, &url, &mut wake) => end,
            () = paused(&mut pause) => End::Paused,
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
        match sleep(wait, &mut wake, &mut pause).await {
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

/// Waits while the app has paused syncing. Returns `false` when the
/// database was dropped meanwhile.
async fn resumed(pause: &mut watch::Receiver<bool>) -> bool {
    while *pause.borrow_and_update() {
        if pause.changed().await.is_err() {
            return false;
        }
    }
    true
}

/// Resolves when the app pauses syncing; never, once the database was
/// dropped, which the session notices on its own.
async fn paused(pause: &mut watch::Receiver<bool>) {
    loop {
        if *pause.borrow_and_update() {
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
    wake: &mut watch::Receiver<()>,
    pause: &mut watch::Receiver<bool>,
) -> Slept {
    let sleep = tokio::time::sleep(wait);
    tokio::pin!(sleep);
    loop {
        tokio::select! {
            () = &mut sleep => return Slept::Elapsed,
            changed = wake.changed() => if changed.is_err() { return Slept::Dropped },
            () = paused(pause) => return Slept::Paused,
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
