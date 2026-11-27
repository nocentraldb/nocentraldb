//! A relay function that gives no URL in time, on the browser's event loop
//! (NOC-585; Carsten, 2026-11-27, NOC-572): the bound of each call is the
//! global's `setTimeout`, as the sync's sleep is, here shorter than 30 s
//! (`relay::with_bound`). A function that never settles runs in Node and
//! in Chromium; the others need the harness's relay, as `sync_tests.rs`
//! says.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use ed25519_dalek::SigningKey;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::sync::{ACCOUNT, RELAY};
use crate::relay::with_bound;
use crate::sync::Connection;
use crate::sync::time::{Instant, sleep};
use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The bound of these tests, in place of 30 s.
const BOUND: Duration = Duration::from_millis(500);

/// How long a test waits.
const WAIT: Duration = Duration::from_secs(20);

async fn open(options: OpenOptions) -> Db {
    let (signing, agreement) = (*crate::keys::random(), *crate::keys::random());
    Db::open_as(
        options,
        signing,
        agreement,
        SigningKey::from_bytes(&ACCOUNT),
    )
    .await
    .expect("a database opens in the browser")
}

/// Waits until `done` holds.
async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "{what}: not in time");
        sleep(Duration::from_millis(20)).await;
    }
}

#[test]
async fn a_relay_function_that_never_settles_shows_as_relay_url_failed_and_is_called_again() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            std::future::pending::<Result<String, ()>>()
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

#[test]
#[ignore = "needs the harness's relay: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn a_late_answer_of_a_call_that_ran_out_of_time_is_ignored() {
    let late = BOUND * 8;
    let calls = Arc::new(AtomicUsize::new(0));
    let first_calls = calls.clone();
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            let first = first_calls.fetch_add(1, Ordering::SeqCst) == 0;
            async move {
                if first {
                    // Long after the bound, and no relay URL: were it
                    // taken, the status would name relay_url_failed again.
                    sleep(late).await;
                    return Ok::<_, ()>("https://relay.example.com/late".to_owned());
                }
                Ok(RELAY.to_owned())
            }
        })
    });
    let start = Instant::now();
    let db = open(options).await;

    eventually("connected", || {
        db.sync().current().connection == Connection::Connected
    })
    .await;
    assert!(start.elapsed() < late, "{:?}", start.elapsed());
    // Past the late answer: it changes nothing.
    sleep(late + BOUND).await;
    let status = db.sync().current();

    assert_eq!(status.connection, Connection::Connected);
    assert_eq!(status.error, None);
    assert!(calls.load(Ordering::SeqCst) >= 2);
}

#[test]
#[ignore = "needs the harness's relay: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn a_relay_function_that_answers_in_time_connects_as_before() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let options = with_bound(BOUND, || {
        OpenOptions::new(LOCK).in_memory().relay_with(move || {
            counted.fetch_add(1, Ordering::SeqCst);
            async {
                sleep(BOUND / 4).await;
                Ok::<_, ()>(RELAY.to_owned())
            }
        })
    });
    let db = open(options).await;

    eventually("connected", || {
        db.sync().current().connection == Connection::Connected
    })
    .await;

    assert_eq!(db.sync().current().error, None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
