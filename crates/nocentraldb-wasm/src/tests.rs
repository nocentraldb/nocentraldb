//! The export answers the bridge's requests, natively and on wasm32.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use nocentraldb::bridge::cbor::{self, Key, Value};

use super::Core;

const LOCK: &str = include_str!("../../nocentraldb/tests/model/model.lock.json");

fn map(entries: &[(&str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(k, v)| (Key::Text((*k).into()), v.clone()))
            .collect(),
    )
}

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("a call waited"),
    }
}

fn request(id: u64, call: &str, args: Value) -> Vec<u8> {
    cbor::encode(&map(&[
        ("id", Value::Unsigned(id)),
        ("call", Value::Text(call.into())),
        ("args", args),
    ]))
}

#[test]
fn the_core_opens_a_database_in_memory_and_creates_a_space() {
    let core = Core::new();
    let open = map(&[
        ("lock", Value::Text(LOCK.into())),
        ("inMemory", Value::Bool(true)),
    ]);
    let answer = cbor::decode(&now(core.answer(request(1, "open", open)))).unwrap();
    let Value::Map(answer) = answer else { panic!() };
    assert_eq!(
        answer.get(&Key::Text("id".into())),
        Some(&Value::Unsigned(1))
    );
    let Some(Value::Map(ok)) = answer.get(&Key::Text("ok".into())) else {
        panic!("{answer:?}")
    };
    assert!(ok.contains_key(&Key::Text("sync".into())), "{ok:?}");

    let answer = cbor::decode(&now(core.answer(request(2, "spaces.create", map(&[]))))).unwrap();
    let Value::Map(answer) = answer else { panic!() };
    let Some(Value::Text(space)) = answer.get(&Key::Text("ok".into())) else {
        panic!("{answer:?}")
    };
    assert_eq!(space.len(), 32);
}

#[test]
fn a_call_before_open_answers_closed() {
    let core = Core::new();
    let answer = cbor::decode(&now(core.answer(request(5, "spaces.list", map(&[]))))).unwrap();
    let Value::Map(answer) = answer else { panic!() };
    let Some(Value::Map(error)) = answer.get(&Key::Text("error".into())) else {
        panic!("{answer:?}")
    };
    assert_eq!(
        error.get(&Key::Text("code".into())),
        Some(&Value::Text("closed".into()))
    );
}

// The worker forwards the fold events of the database the core opened
// (NOC-176): the core gives them once open, and gives none before.
#[test]
fn the_core_gives_the_events_of_the_open_database() {
    let core = Core::new();
    assert!(core.events().is_none(), "no database yet");
    let open = map(&[
        ("lock", Value::Text(LOCK.into())),
        ("inMemory", Value::Bool(true)),
    ]);
    now(core.answer(request(1, "open", open)));
    let events = core.events().expect("the open database's events");
    now(core.answer(request(2, "spaces.create", map(&[]))));
    let first = core.events().expect("still open");
    assert!(
        first.same(&events),
        "the same feed while the database is open"
    );
    now(core.answer(request(3, "close", map(&[]))));
    assert!(events.is_closed());
}
