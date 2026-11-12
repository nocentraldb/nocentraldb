//! The SDK on wasm32, in Node (NOC-175): a database in memory opens and
//! writes, and one opened with a relay syncs in the background over the
//! platform's WebSocket; with no relay to reach, it reports itself offline.

use wasm_bindgen_test::wasm_bindgen_test as test;

use std::time::Duration;

use crate::sync::Connection;
use crate::sync::time::{Instant, sleep};
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
async fn a_database_with_a_relay_opens_and_is_offline_while_no_relay_answers() {
    let options = OpenOptions::new(LOCK).in_memory().relay("ws://127.0.0.1:1");
    let db = now(Db::open(options)).expect("a database with a relay opens on wasm32");
    assert_eq!(db.sync().current().connection, Connection::Connecting);
    let start = Instant::now();
    while db.sync().current().connection != Connection::Offline {
        assert!(start.elapsed() < Duration::from_secs(10), "never offline");
        sleep(Duration::from_millis(20)).await;
    }
}

#[test]
fn a_relay_url_that_is_not_a_websocket_url_is_refused() {
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay("https://relay.example");
    let refused = now(Db::open(options)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
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
