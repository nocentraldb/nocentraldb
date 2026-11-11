//! What every test of the SDK shares: the test model's lock, and running
//! the SDK's futures. Every call of the SDK finishes on the device without
//! waiting (RFC 0009, "One write, one change"), so a future is polled once
//! and must be ready; a subscription's next page is polled the same way.
#![allow(dead_code)]

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use futures_core::Stream;
use nocentraldb::{Db, OpenOptions};

/// The lock of `tests/model/model.ts`.
pub const LOCK: &str = include_str!("../model/model.lock.json");

/// The output of `future`, which must be ready at its first poll.
pub fn now<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the future waits, but the SDK never waits on the device"),
    }
}

/// The item `stream` has ready now, or `None` when it has none yet.
pub fn ready<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    let mut cx = Context::from_waker(Waker::noop());
    match std::pin::Pin::new(stream).poll_next(&mut cx) {
        Poll::Ready(Some(item)) => Some(item),
        Poll::Ready(None) => panic!("the subscription ended"),
        Poll::Pending => None,
    }
}

/// Waits, a few seconds at most, until `db` runs no rebuild of its view
/// (RFC 0009, `db.view`), and returns every status it saw on the way.
pub fn rebuilt(db: &Db) -> Vec<nocentraldb::ViewStatus> {
    let mut seen = Vec::new();
    for _ in 0..500 {
        let status = db.view().current();
        seen.push(status.clone());
        if status.rebuild.is_none() {
            return seen;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the rebuild did not end: {seen:?}");
}

/// A database in memory, opened from the test model's lock.
pub fn open() -> Db {
    now(Db::open(OpenOptions::new(LOCK).in_memory())).expect("the test lock opens")
}

/// A directory of its own under the system's temporary directory, removed
/// with everything in it when dropped.
pub struct TempDir(pub std::path::PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let unique = format!(
            "nocentraldb-{name}-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        TempDir(dir)
    }

    pub fn join(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
