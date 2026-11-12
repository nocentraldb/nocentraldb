//! The largest page (RFC 0009, "Queries and indexes": `take` is at most
//! 1000, and a larger value fails with `invalidValue`, in both SDKs;
//! Carsten, 2026-11-12, NOC-274).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::error::ErrorCode;
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

fn db_with_tasks(n: i64) -> Db {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    now(project.write(|tx| {
        for priority in 0..n {
            tx.create("tasks", [("priority", Value::from(priority))])?;
        }
        Ok(())
    }))
    .unwrap();
    db
}

#[test]
fn a_page_of_1000_is_taken() {
    let db = db_with_tasks(3);
    let query = db.index("tasks", "byPriority").unwrap().take(1000);
    let page = now(db.find(query)).unwrap();
    assert_eq!(page.items.len(), 3);
}

#[test]
fn a_page_above_1000_is_refused_with_invalid_value() {
    let db = db_with_tasks(1);
    let query = db.index("tasks", "byPriority").unwrap().take(1001);
    let refused = now(db.find(query)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
fn a_page_of_0_is_refused_with_invalid_value() {
    let db = db_with_tasks(1);
    let query = db.index("tasks", "byPriority").unwrap().take(0);
    let refused = now(db.find(query)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}
