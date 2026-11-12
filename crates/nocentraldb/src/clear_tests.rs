//! Clearing a field (RFC 0009, "The Rust SDK": `Value::clear()`, as `null`
//! clears a field in TypeScript; Carsten, 2026-11-12, NOC-277): a set (1)
//! with no value, for a basic value, a link or a reference. A set or a
//! list of links has nothing to clear in one value.

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

#[test]
fn a_clear_empties_a_basic_value() {
    let (_db, project) = space();
    let id = now(project.create(
        "tasks",
        [("title", Value::from("a")), ("priority", Value::from(3))],
    ))
    .unwrap()
    .ids[0];

    now(project.update(
        "tasks",
        id,
        [("title", Value::clear()), ("priority", Value::clear())],
    ))
    .unwrap();

    let task = now(project.get("tasks", id)).unwrap().unwrap();
    assert_eq!(task.get::<&str>("title").unwrap(), None);
    assert_eq!(task.get::<i64>("priority").unwrap(), None);
}

#[test]
fn a_clear_empties_a_link() {
    let (_db, project) = space();
    let target = now(project.create("projects", [("name", Value::from("p"))]))
        .unwrap()
        .ids[0];
    let id = now(project.create("tasks", [("project", Value::from(target))]))
        .unwrap()
        .ids[0];

    now(project.update("tasks", id, [("project", Value::clear())])).unwrap();

    let task = now(project.get("tasks", id)).unwrap().unwrap();
    assert_eq!(task.get::<ObjectId>("project").unwrap(), None);
}

#[test]
fn a_create_with_a_clear_leaves_the_field_unset() {
    let (_db, project) = space();
    let id = now(project.create(
        "tasks",
        [("title", Value::clear()), ("priority", Value::from(1))],
    ))
    .unwrap()
    .ids[0];

    let task = now(project.get("tasks", id)).unwrap().unwrap();
    assert_eq!(task.get::<&str>("title").unwrap(), None);
    assert_eq!(task.get::<i64>("priority").unwrap(), Some(1));
}

#[test]
fn a_clear_of_a_set_is_refused() {
    let (_db, project) = space();
    let id = now(project.create("tasks", [("tags", Value::add(["a"]))]))
        .unwrap()
        .ids[0];

    let refused = now(project.update("tasks", id, [("tags", Value::clear())])).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}
