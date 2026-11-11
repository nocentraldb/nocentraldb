//! What the SDK does on wasm32 before syncing in the browser is built
//! (NOC-175): a database opened with a relay is refused, with a message
//! that says so, rather than opened without syncing.

use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::{Db, ErrorCode, OpenOptions};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

#[test]
fn a_database_with_a_relay_is_refused_until_the_browser_syncs() {
    let options = OpenOptions::new(LOCK).in_memory().relay("ws://127.0.0.1:1");
    let refused = now(Db::open(options)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert!(refused.to_string().contains("not built"), "{refused}");
}

#[test]
fn a_database_in_memory_opens_and_writes() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", crate::Value::from("on wasm32"))])?;
        Ok(())
    }))
    .unwrap();
    assert!(now(project.get("tasks", written.ids[0])).unwrap().is_some());
}
