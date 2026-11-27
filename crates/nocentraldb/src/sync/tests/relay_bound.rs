//! A relay function that gives no URL in time (NOC-585; Carsten,
//! 2026-11-27, NOC-572): the SDK waits 30 s for each call of the app's
//! function, a shorter bound here (`relay::with_bound`). After it the
//! attempt ends as `relay_url_failed`, the SDK retries with backoff and
//! calls the function again, and a late answer of the old call is ignored.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::gate::Gate;
use super::support::*;
use crate::relay::with_bound;
use crate::sync::Connection;
use crate::{Db, OpenOptions};

const ME: Who = 1;

/// The bound of these tests, in place of 30 s.
const BOUND: Duration = Duration::from_millis(300);

async fn open(options: OpenOptions) -> Db {
    Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_function_that_never_settles_shows_as_relay_url_failed_and_is_called_again() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            std::future::pending::<Result<String, std::io::Error>>()
        })
    });
    let start = Instant::now();
    let db = open(options).await;

    let mut status = db.sync().current();
    eventually("the status names the function that gave no URL", || {
        status = db.sync().current();
        status.error.is_some()
    })
    .await;
    assert!(start.elapsed() >= BOUND, "{:?}", start.elapsed());
    assert_eq!(status.error.as_deref(), Some("relay_url_failed"));
    assert_ne!(status.connection, Connection::Connected);
    eventually("the function was called again", || {
        calls.load(Ordering::SeqCst) >= 2
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_late_answer_of_a_call_that_ran_out_of_time_is_ignored() {
    let relay = LocalRelay::start().await;
    let gate = Gate::start(relay.addr, "fresh").await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (base, counted) = (gate.url(), calls.clone());
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            let first = counted.fetch_add(1, Ordering::SeqCst) == 0;
            let (answer, answered) = tokio::sync::oneshot::channel();
            let token = if first { "late" } else { "fresh" };
            let url = format!("{base}/prod?token={token}");
            // The first call answers long after the bound, the others at once.
            std::thread::spawn(move || {
                if first {
                    std::thread::sleep(BOUND * 4);
                }
                let _ = answer.send(url);
            });
            async move { answered.await.map_err(|_| std::io::Error::other("gone")) }
        })
    });
    let db = open(options).await;

    let status = synced(&db).await;
    // Past the late answer: it changes nothing.
    tokio::time::sleep(BOUND * 5).await;

    assert_eq!(status.error, None);
    assert_eq!(db.sync().current().connection, Connection::Connected);
    let targets = gate.targets();
    assert!(
        !targets.iter().any(|target| target.contains("late")),
        "{targets:?}"
    );
    assert!(calls.load(Ordering::SeqCst) >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_function_that_answers_in_time_connects_as_before() {
    let relay = LocalRelay::start().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (url, counted) = (relay.url(), calls.clone());
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            let url = url.clone();
            async move {
                tokio::time::sleep(BOUND / 4).await;
                Ok::<_, std::io::Error>(url)
            }
        })
    });
    let db = open(options).await;

    let status = synced(&db).await;

    assert_eq!(status.error, None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
