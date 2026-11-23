//! The app's admission function through the bridge (RFC 0015, "In the
//! SDK"; Carsten, 2026-11-23, NOC-493): `open` takes
//! `"admission": true`; when the sync needs an admission, the worker sends
//! `{"event": "admission", "request": n, "identity": hex}`, and the SDK
//! answers with `admission.answer` `{"request": n, "admission": text |
//! null}`, which gives the sync that text, or nothing.

use ncdb_core::cbor::{self, Value};

use super::{Bridge, LOCK, field, map, ok, text};

/// Runs `future` to its end on a runtime of its own: the fetch waits for
/// the answer.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(future)
}

fn opened(admission: bool) -> Bridge {
    let mut bridge = Bridge::new();
    let mut args = vec![("lock", text(LOCK)), ("inMemory", Value::Bool(true))];
    if admission {
        args.push(("admission", Value::Bool(true)));
    }
    ok(&mut bridge, "open", map(&args));
    bridge
}

/// The `admission` events the worker sends now.
fn asked(bridge: &Bridge) -> Vec<Value> {
    let events = bridge.events().expect("an open database");
    events
        .take()
        .into_iter()
        .map(|bytes| cbor::decode(&bytes).unwrap())
        .filter(|event| field(event, "event") == Some(&text("admission")))
        .collect()
}

#[test]
fn the_sync_asks_the_sdk_and_gets_its_answer() {
    let mut bridge = opened(true);
    let fetch = bridge.db.as_ref().unwrap().lock().admission.clone();
    let fetch = fetch.expect("open set the function");
    let identity = "ab".repeat(32);
    let pending = std::thread::spawn({
        let identity = identity.clone();
        move || block_on(fetch(identity))
    });
    let mut events = Vec::new();
    for _ in 0..200 {
        events = asked(&bridge);
        if !events.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let [event] = events.as_slice() else {
        panic!("one admission event: {events:?}")
    };
    assert_eq!(field(event, "identity"), Some(&text(&identity)));
    let request = field(event, "request").expect("a request").clone();
    let answer = ok(
        &mut bridge,
        "admission.answer",
        map(&[("request", request), ("admission", text("ncdb-admit-1:AA"))]),
    );
    assert_eq!(answer, Value::Null);
    assert_eq!(pending.join().unwrap().as_deref(), Some("ncdb-admit-1:AA"));
}

#[test]
fn a_null_answer_gives_the_sync_nothing() {
    let mut bridge = opened(true);
    let fetch = bridge
        .db
        .as_ref()
        .unwrap()
        .lock()
        .admission
        .clone()
        .unwrap();
    let pending = std::thread::spawn(move || block_on(fetch("cd".repeat(32))));
    let request = loop {
        if let Some(event) = asked(&bridge).pop() {
            break field(&event, "request").unwrap().clone();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    ok(
        &mut bridge,
        "admission.answer",
        map(&[("request", request), ("admission", Value::Null)]),
    );
    assert_eq!(pending.join().unwrap(), None);
    // An answer to a request nobody waits for is ignored.
    let late = ok(
        &mut bridge,
        "admission.answer",
        map(&[("request", Value::Unsigned(99)), ("admission", Value::Null)]),
    );
    assert_eq!(late, Value::Null);
}

#[test]
fn without_the_flag_the_database_has_no_function() {
    let bridge = opened(false);
    assert!(bridge.db.as_ref().unwrap().lock().admission.is_none());
}
