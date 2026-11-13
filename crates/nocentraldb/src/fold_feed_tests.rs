//! The fold feed (NOC-176): the space and collections of every fold, for
//! the TypeScript SDK's worker, which re-runs the reads its subscriptions
//! follow (RFC 0009, "Subscribe"). It carries names and ids, never values.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::value::Value;
use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

#[test]
fn a_write_puts_its_space_and_collection_in_the_feed() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let feed = db.folds();
    let project = now(db.spaces().create()).unwrap();
    feed.take();
    now(project.create("tasks", [("title", Value::from("a"))])).unwrap();

    let folds = feed.take();
    assert_eq!(folds.len(), 1, "{folds:?}");
    assert_eq!(folds[0].0, project.id());
    assert_eq!(folds[0].1, vec!["tasks".to_owned()]);
    assert!(feed.take().is_empty(), "take empties the feed");
}

#[test]
fn the_feed_wakes_a_waiter_when_a_fold_lands() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    let feed = db.folds();
    feed.take();

    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    let mut waiting = Box::pin(feed.wait());
    assert!(
        waiting.as_mut().poll(&mut cx).is_pending(),
        "nothing folded yet"
    );
    now(project.create("tasks", [("title", Value::from("b"))])).unwrap();
    assert!(waiting.as_mut().poll(&mut cx).is_ready(), "a fold wakes it");
}

#[test]
fn a_closed_feed_stops_waiting() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let feed = db.folds();
    feed.take();
    feed.close();
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(Box::pin(feed.wait()).as_mut().poll(&mut cx).is_ready());
    assert!(feed.is_closed());
}
