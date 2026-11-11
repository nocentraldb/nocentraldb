//! The SDK writes the writer's model, the `model.id` and `model.version`
//! of the lock it opened the database with, into payload key 64 of every
//! change it seals (RFC 0002, "The writer's model").

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::WriterModel;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::receive::Checks;

use crate::{Db, OpenOptions, Value};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    }
}

#[test]
fn a_written_change_names_the_lock_that_wrote_it() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    let written = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("named"))])?;
        Ok(())
    }))
    .unwrap();

    let inner = db.lock();
    let log = inner.view.log(space.id().0).unwrap();
    let change = log.get(log.position_of(written.change.0).unwrap()).unwrap();
    let payload = inner.spaces[&space.id().0]
        .checks
        .open(change)
        .expect("the member opens its own change");

    let lock = ModelLock::from_json(LOCK).unwrap();
    assert_eq!(
        payload.writer_model(),
        Some(Ok(WriterModel {
            id: lock.model.id.0,
            version: lock.model.version,
        }))
    );
}
