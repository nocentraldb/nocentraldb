//! What one large write through the bridge holds in memory (NOC-422): the
//! write a browser tab's worker answers for the TypeScript SDK. The
//! request comes as CBOR; the bridge must not hold the request's tree, the
//! records parsed from it and the transaction's copy of them, all of them
//! whole, next to each other. It holds what a write through the Rust SDK
//! holds (tests/memory_write.rs of the crate nocentraldb): at most ten times
//! the change's bytes.

#[path = "../../nocentraldb/tests/support/peak.rs"]
mod peak;

use nocentraldb::bridge::Bridge;
use nocentraldb::bridge::cbor::{self, Key, Value};

#[global_allocator]
static COUNTING: peak::Counting = peak::Counting;

const LOCK: &str = include_str!("../../nocentraldb/tests/model/model.lock.json");

fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Key::Text(key.into()), value))
            .collect(),
    )
}

/// Polls `future`, which must not wait.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("a bridge call waited"),
    }
}

/// The answer's `ok` value to `call` with `args`.
fn ok(bridge: &mut Bridge, call: &str, args: Value) -> Value {
    let request = map(vec![
        ("id", Value::Unsigned(1)),
        ("call", Value::Text(call.into())),
        ("args", args),
    ]);
    match cbor::decode(&now(bridge.handle(&cbor::encode(&request)))).unwrap() {
        Value::Map(mut answer) => answer
            .remove(&Key::Text("ok".into()))
            .unwrap_or_else(|| panic!("{call} failed: {answer:?}")),
        other => panic!("{other:?}"),
    }
}

/// The bytes of a change for one record of [`create`], as the bulk import
/// measured them (docs/bulk-import.md).
const CHANGE_BYTES_PER_RECORD: usize = 390;

/// A create of the `i`th record, as the bulk import measurement writes it.
fn create(i: u64) -> Value {
    let status = if i.is_multiple_of(3) { "done" } else { "open" };
    let tags = Value::Array(vec![
        Value::Text("imported".into()),
        Value::Text(if i.is_multiple_of(2) { "even" } else { "odd" }.into()),
    ]);
    map(vec![
        ("op", Value::Text("create".into())),
        ("collection", Value::Text("tasks".into())),
        (
            "values",
            map(vec![
                (
                    "title",
                    Value::Text(format!("Imported task {i:06}: follow up on the old record")),
                ),
                ("status", Value::Text(status.into())),
                ("priority", Value::Unsigned(i % 5)),
                ("estimate", Value::Float(0.5 * (i % 8) as f64)),
                ("urgent", Value::Bool(i.is_multiple_of(10))),
                ("tags", map(vec![("$add", tags)])),
            ]),
        ),
    ])
}

const RECORDS: u64 = 2_000;

#[test]
fn a_write_through_the_bridge_holds_what_a_write_through_the_rust_sdk_holds() {
    let mut bridge = Bridge::new();
    ok(
        &mut bridge,
        "open",
        map(vec![
            ("lock", Value::Text(LOCK.into())),
            ("inMemory", Value::Bool(true)),
        ]),
    );
    let space = ok(&mut bridge, "spaces.create", map(vec![]));
    let request = cbor::encode(&map(vec![
        ("id", Value::Unsigned(2)),
        ("call", Value::Text("write".into())),
        (
            "args",
            map(vec![
                ("space", space),
                ("ops", Value::Array((0..RECORDS).map(create).collect())),
            ]),
        ),
    ]));
    let change = RECORDS as usize * CHANGE_BYTES_PER_RECORD;
    let (answer, held) = peak::peak(|| now(bridge.handle(&request)));
    assert!(
        matches!(cbor::decode(&answer), Ok(Value::Map(ref m)) if m.contains_key(&Key::Text("ok".into()))),
        "the write failed"
    );
    assert!(
        held.peak <= 10 * change,
        "a write of a {change}-byte change held {:.1} times as many at its peak",
        held.peak as f64 / change as f64
    );
}
