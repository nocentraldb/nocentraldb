//! The one-call shorthands of `space.write` (RFC 0009, "One write, one
//! change": `project.create`, `project.update`, `project.delete` and
//! `project.restore`; NOC-277): each is a `write` with one call, so one
//! change, and answers what that write answers.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::error::ErrorCode;
use crate::id::ObjectId;
use crate::space::Space;
use crate::value::Value;
use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// Polls `future`, which must not wait.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

fn space() -> (Db, Space) {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    (db, project)
}

/// How many changes the log of `space` holds.
fn changes(db: &Db, space: &Space) -> usize {
    let inner = db.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

fn title(space: &Space, id: ObjectId) -> Option<String> {
    let task = now(space.get("tasks", id)).unwrap()?;
    task.get::<&str>("title").unwrap().map(str::to_owned)
}

fn deleted(space: &Space, id: ObjectId) -> bool {
    now(space.get("tasks", id)).unwrap().unwrap().deleted()
}

#[test]
fn each_shorthand_writes_one_change() {
    let (db, project) = space();
    let before = changes(&db, &project);

    let created = now(project.create("tasks", [("title", Value::from("a"))])).unwrap();
    assert_eq!(created.ids.len(), 1);
    let id = created.ids[0];
    assert_eq!(changes(&db, &project), before + 1);
    assert_eq!(title(&project, id).as_deref(), Some("a"));

    let updated = now(project.update("tasks", id, [("title", Value::from("b"))])).unwrap();
    assert!(updated.ids.is_empty());
    assert_ne!(updated.change, created.change);
    assert_eq!(changes(&db, &project), before + 2);
    assert_eq!(title(&project, id).as_deref(), Some("b"));

    now(project.delete("tasks", id)).unwrap();
    assert_eq!(changes(&db, &project), before + 3);
    assert!(deleted(&project, id));

    now(project.restore("tasks", id)).unwrap();
    assert_eq!(changes(&db, &project), before + 4);
    assert!(!deleted(&project, id));
    assert_eq!(title(&project, id).as_deref(), Some("b"));
}

#[test]
fn a_shorthand_refuses_what_write_refuses_and_writes_nothing() {
    let (db, project) = space();
    let before = changes(&db, &project);
    let unknown: ObjectId = "00112233445566778899aabbccddeeff".parse().unwrap();

    let update = now(project.update("tasks", unknown, [("title", Value::from("x"))]));
    let delete = now(project.delete("tasks", unknown));
    let restore = now(project.restore("tasks", unknown));
    let kind = now(project.create("tasks", [("priority", Value::from("high"))]));

    assert_eq!(update.unwrap_err().code(), ErrorCode::NotFound);
    assert_eq!(delete.unwrap_err().code(), ErrorCode::NotFound);
    assert_eq!(restore.unwrap_err().code(), ErrorCode::NotFound);
    assert_eq!(kind.unwrap_err().code(), ErrorCode::InvalidValue);
    assert_eq!(changes(&db, &project), before);
}
