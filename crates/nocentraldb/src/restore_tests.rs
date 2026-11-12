//! Restoring a deleted object (RFC 0009, "One write, one change":
//! `tx.restore(collection, id)` is delete (4) with `false`; RFC 0004,
//! "Creating and deleting an object"; NOC-277). A restore of an object the
//! copy does not hold is refused with `NotFound`, as an update or a delete
//! is (RFC 0009, "What the SDK refuses").

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

/// A database in memory with one space and one task in it.
fn task() -> (Db, Space, ObjectId) {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    let id = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("Keep me"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    (db, project, id)
}

fn deleted(project: &Space, id: ObjectId) -> bool {
    now(project.get("tasks", id))
        .unwrap()
        .expect("the copy holds the task")
        .deleted()
}

#[test]
fn a_deleted_object_is_restored_with_its_fields() {
    let (_db, project, id) = task();
    now(project.write(|tx| tx.delete("tasks", id))).unwrap();
    assert!(deleted(&project, id));

    now(project.write(|tx| tx.restore("tasks", id))).unwrap();

    assert!(!deleted(&project, id));
    let task = now(project.get("tasks", id)).unwrap().unwrap();
    assert_eq!(task.get::<&str>("title").unwrap(), Some("Keep me"));
}

#[test]
fn a_restore_of_an_object_that_is_not_deleted_keeps_it() {
    let (_db, project, id) = task();
    now(project.write(|tx| tx.restore("tasks", id))).unwrap();
    assert!(!deleted(&project, id));
}

#[test]
fn a_delete_and_a_restore_in_one_write_leave_the_object_restored() {
    let (_db, project, id) = task();
    now(project.write(|tx| {
        tx.delete("tasks", id)?;
        tx.restore("tasks", id)
    }))
    .unwrap();
    assert!(!deleted(&project, id));
}

#[test]
fn a_restore_of_an_object_the_copy_does_not_hold_is_refused() {
    let (_db, project, _) = task();
    let unknown: ObjectId = "00112233445566778899aabbccddeeff".parse().unwrap();
    let refused = now(project.write(|tx| tx.restore("tasks", unknown))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotFound);
}

#[test]
fn a_restore_in_a_collection_the_model_does_not_have_is_refused() {
    let (_db, project, id) = task();
    let refused = now(project.write(|tx| tx.restore("people", id))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotFound);
}
