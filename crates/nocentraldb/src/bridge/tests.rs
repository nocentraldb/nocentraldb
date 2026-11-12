//! The bridge's message protocol (NOC-176, slice 1), natively on a
//! database in memory: what the worker answers the TypeScript SDK.

use ncdb_core::cbor::{self, Key, Value};

use super::Bridge;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn map(entries: &[(&str, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(k, v)| (Key::Text((*k).into()), v.clone()))
            .collect(),
    )
}

fn field<'a>(value: &'a Value, name: &str) -> Option<&'a Value> {
    match value {
        Value::Map(map) => map.get(&Key::Text(name.into())),
        _ => None,
    }
}

fn as_text(value: &Value) -> &str {
    match value {
        Value::Text(text) => text,
        other => panic!("not text: {other:?}"),
    }
}

/// Polls `future`, which must not wait.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("a bridge call waited"),
    }
}

/// Sends `call` with `args` as request `id`; the whole answer.
fn send(bridge: &mut Bridge, id: u64, call: &str, args: Value) -> Value {
    let request = map(&[
        ("id", Value::Unsigned(id)),
        ("call", text(call)),
        ("args", args),
    ]);
    let answer = cbor::decode(&now(bridge.handle(&cbor::encode(&request)))).expect("CBOR");
    assert_eq!(
        field(&answer, "id"),
        Some(&Value::Unsigned(id)),
        "{answer:?}"
    );
    answer
}

/// The `ok` value of a call that must succeed.
fn ok(bridge: &mut Bridge, call: &str, args: Value) -> Value {
    let answer = send(bridge, 7, call, args);
    field(&answer, "ok")
        .unwrap_or_else(|| panic!("{call} failed: {answer:?}"))
        .clone()
}

/// The error code of a call that must fail.
fn code(bridge: &mut Bridge, call: &str, args: Value) -> String {
    let answer = send(bridge, 9, call, args);
    let error = field(&answer, "error").unwrap_or_else(|| panic!("{call} succeeded: {answer:?}"));
    as_text(field(error, "code").expect("a code")).to_owned()
}

fn opened() -> Bridge {
    let mut bridge = Bridge::new();
    let answer = ok(
        &mut bridge,
        "open",
        map(&[("lock", text(LOCK)), ("inMemory", Value::Bool(true))]),
    );
    assert_eq!(answer, Value::Null);
    bridge
}

#[test]
fn a_space_is_created_got_and_listed() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let id = as_text(&space).to_owned();
    assert_eq!(id.len(), 32);
    assert!(
        id.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );
    assert_eq!(
        ok(&mut bridge, "spaces.get", map(&[("space", text(&id))])),
        text(&id)
    );
    let list = ok(&mut bridge, "spaces.list", map(&[]));
    let Value::Array(list) = list else {
        panic!("{list:?}")
    };
    assert_eq!(list.len(), 1);
    assert_eq!(field(&list[0], "id"), Some(&text(&id)));
    assert_eq!(field(&list[0], "role"), Some(&text("admin")));
}

#[test]
fn a_write_creates_and_a_get_reads_it_back() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let values = map(&[
        ("title", text("Write the bridge")),
        ("status", text("open")),
        ("priority", Value::Unsigned(2)),
        ("urgent", Value::Bool(true)),
        (
            "tags",
            map(&[("$add", Value::Array(vec![text("rust"), text("ts")]))]),
        ),
    ]);
    let written = ok(
        &mut bridge,
        "write",
        map(&[
            ("space", space.clone()),
            (
                "ops",
                Value::Array(vec![map(&[
                    ("op", text("create")),
                    ("collection", text("tasks")),
                    ("values", values),
                ])]),
            ),
        ]),
    );
    assert_eq!(
        as_text(field(&written, "change").expect("a change")).len(),
        64
    );
    let Some(Value::Array(ids)) = field(&written, "ids") else {
        panic!("{written:?}")
    };
    assert_eq!(ids.len(), 1);

    let read = ok(
        &mut bridge,
        "get",
        map(&[
            ("space", space.clone()),
            ("collection", text("tasks")),
            ("id", ids[0].clone()),
        ]),
    );
    assert_eq!(field(&read, "$id"), Some(&ids[0]));
    assert_eq!(field(&read, "$space"), Some(&space));
    assert_eq!(field(&read, "$deleted"), Some(&Value::Bool(false)));
    let fields = field(&read, "fields").expect("its fields");
    assert_eq!(field(fields, "title"), Some(&text("Write the bridge")));
    assert_eq!(field(fields, "priority"), Some(&Value::Unsigned(2)));
    assert_eq!(field(fields, "urgent"), Some(&Value::Bool(true)));
    assert_eq!(
        field(fields, "tags"),
        Some(&Value::Array(vec![text("rust"), text("ts")]))
    );
}

#[test]
fn an_update_and_a_delete_change_what_a_get_reads() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let op = |op: &str, id: Option<&Value>, values: Option<Value>| {
        let mut entries = vec![("op", text(op)), ("collection", text("tasks"))];
        if let Some(id) = id {
            entries.push(("id", id.clone()));
        }
        if let Some(values) = values {
            entries.push(("values", values));
        }
        map(&entries)
    };
    let write = |bridge: &mut Bridge, op: Value| {
        ok(
            bridge,
            "write",
            map(&[("space", space.clone()), ("ops", Value::Array(vec![op]))]),
        )
    };
    let created = write(
        &mut bridge,
        op("create", None, Some(map(&[("title", text("a"))]))),
    );
    let Some(Value::Array(ids)) = field(&created, "ids") else {
        panic!()
    };
    let id = ids[0].clone();
    let get = |bridge: &mut Bridge| {
        ok(
            bridge,
            "get",
            map(&[
                ("space", space.clone()),
                ("collection", text("tasks")),
                ("id", id.clone()),
            ]),
        )
    };

    write(
        &mut bridge,
        op("update", Some(&id), Some(map(&[("title", text("b"))]))),
    );
    assert_eq!(
        field(field(&get(&mut bridge), "fields").unwrap(), "title"),
        Some(&text("b"))
    );
    // `null`, which clears a field in RFC 0009, waits for the Rust SDK to
    // write a clear: until then the bridge refuses it.
    let clear = map(&[
        ("space", space.clone()),
        (
            "ops",
            Value::Array(vec![op(
                "update",
                Some(&id),
                Some(map(&[("title", Value::Null)])),
            )]),
        ),
    ]);
    assert_eq!(code(&mut bridge, "write", clear), "invalidValue");
    write(&mut bridge, op("delete", Some(&id), None));
    assert_eq!(
        field(&get(&mut bridge), "$deleted"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn a_get_of_an_unknown_object_answers_null() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let unknown = text(&"ab".repeat(16));
    let read = ok(
        &mut bridge,
        "get",
        map(&[
            ("space", space),
            ("collection", text("tasks")),
            ("id", unknown),
        ]),
    );
    assert_eq!(read, Value::Null);
}

#[test]
fn errors_carry_rfc_0009s_codes() {
    let mut bridge = opened();
    let unknown = text(&"cd".repeat(16));
    assert_eq!(
        code(&mut bridge, "spaces.get", map(&[("space", unknown)])),
        "notFound"
    );
    assert_eq!(
        code(&mut bridge, "spaces.get", map(&[("space", text("nope"))])),
        "invalidValue"
    );
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let bad_field = map(&[
        ("space", space),
        (
            "ops",
            Value::Array(vec![map(&[
                ("op", text("create")),
                ("collection", text("tasks")),
                ("values", map(&[("nosuchfield", text("x"))])),
            ])]),
        ),
    ]);
    assert_eq!(code(&mut bridge, "write", bad_field), "invalidValue");
    assert_eq!(code(&mut bridge, "frobnicate", map(&[])), "invalidValue");
    assert_eq!(
        code(
            &mut bridge,
            "open",
            map(&[("lock", text(LOCK)), ("inMemory", Value::Bool(true))])
        ),
        "invalidValue"
    );
}

#[test]
fn after_close_every_call_but_open_fails_with_closed() {
    let mut bridge = opened();
    assert_eq!(ok(&mut bridge, "close", map(&[])), Value::Null);
    assert_eq!(code(&mut bridge, "spaces.list", map(&[])), "closed");
    assert_eq!(code(&mut bridge, "spaces.create", map(&[])), "closed");
}

#[test]
fn a_call_before_open_fails_with_closed() {
    let mut bridge = Bridge::new();
    assert_eq!(code(&mut bridge, "spaces.list", map(&[])), "closed");
}

#[test]
fn a_request_that_is_not_one_fails_with_invalid_value_and_id_0() {
    let mut bridge = Bridge::new();
    let answer = cbor::decode(&now(bridge.handle(b"not cbor"))).expect("CBOR");
    assert_eq!(field(&answer, "id"), Some(&Value::Unsigned(0)));
    let error = field(&answer, "error").expect("an error");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
}
