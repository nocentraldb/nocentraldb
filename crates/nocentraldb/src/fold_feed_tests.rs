//! The fold feed (NOC-176): the space and collections of every fold, for
//! the TypeScript SDK's worker, which re-runs the reads its subscriptions
//! follow, and the ids each fold touched with their origin, for
//! `db.changes` (RFC 0009, "Subscribe"; RFC 0006, "Folding", step 6). It
//! carries names and ids, never values.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::subscribe::Origin;
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
    assert_eq!(folds[0].space, project.id());
    assert_eq!(folds[0].collections, vec!["tasks".to_owned()]);
    assert!(feed.take().is_empty(), "take empties the feed");
}

// db.changes (RFC 0009, "Subscribe"): the ids each fold touched, per
// collection, and whether it was this member's own write.
#[test]
fn a_write_names_the_objects_it_touched_as_local() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    let feed = db.folds();
    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("a"))])?;
        tx.create("tasks", [("title", Value::from("b"))])?;
        Ok(())
    }))
    .unwrap();

    let folds = feed.take();
    assert_eq!(folds.len(), 1, "{folds:?}");
    let changes = &folds[0].changes;
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].collection, "tasks");
    assert_eq!(changes[0].origin, Origin::Local);
    let mut ids = changes[0].ids.clone();
    ids.sort();
    let mut expected = written.ids.clone();
    expected.sort();
    assert_eq!(ids, expected);
}

#[test]
fn an_update_names_only_the_object_it_wrote() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    let a = now(project.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    now(project.create("tasks", [("title", Value::from("b"))])).unwrap();
    let feed = db.folds();
    now(project.update("tasks", a, [("title", Value::from("c"))])).unwrap();
    let folds = feed.take();
    assert_eq!(folds[0].changes[0].ids, vec![a]);
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
