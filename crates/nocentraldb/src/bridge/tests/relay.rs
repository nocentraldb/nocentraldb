//! The app's relay function through the bridge (NOC-496): `open` takes
//! `"relay": true`; before each connect the worker sends `{"event":
//! "relay", "request": n}`, and the SDK answers with `relay.answer`
//! `{"request": n, "relay": text | null}`, the URL to connect with, or
//! nothing when the app's function failed.

use std::time::{Duration, Instant};

use ncdb_core::cbor::{self, Value};

use super::{Bridge, LOCK, field, map, ok, text};
use crate::sync::tests::gate::Gate;
use crate::sync::tests::support::LocalRelay;

/// Serves the worker's events until its sync status is connected: answers
/// each `relay` ask with what `answer` gives for the asks so far.
fn serve_until_connected(bridge: &mut Bridge, mut answer: impl FnMut(usize) -> Value) -> usize {
    let start = Instant::now();
    let mut asks = 0;
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "not connected in time"
        );
        let events = bridge.events().expect("an open database").take();
        for event in events.iter().map(|bytes| cbor::decode(bytes).unwrap()) {
            match field(&event, "event") {
                Some(Value::Text(kind)) if kind == "relay" => {
                    let request = field(&event, "request").expect("a request").clone();
                    let relay = answer(asks);
                    asks += 1;
                    let answered = ok(
                        bridge,
                        "relay.answer",
                        map(&[("request", request), ("relay", relay)]),
                    );
                    assert_eq!(answered, Value::Null);
                }
                Some(Value::Text(kind)) if kind == "sync" => {
                    let status = field(&event, "status").expect("a status");
                    if field(status, "connection") == Some(&text("connected")) {
                        return asks;
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn open_with_relay_function() -> Bridge {
    let mut bridge = Bridge::new();
    ok(
        &mut bridge,
        "open",
        map(&[
            ("lock", text(LOCK)),
            ("inMemory", Value::Bool(true)),
            ("relay", Value::Bool(true)),
        ]),
    );
    bridge
}

#[test]
fn after_a_refused_connect_the_worker_asks_the_sdk_for_a_fresh_url() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let relay = runtime.block_on(LocalRelay::start());
    let gate = runtime.block_on(Gate::start(relay.addr, "fresh"));
    let mut bridge = open_with_relay_function();

    let asks = serve_until_connected(&mut bridge, |asked| {
        let token = if asked == 0 { "expired" } else { "fresh" };
        text(&format!("{}/prod?token={token}", gate.url()))
    });

    assert!(asks >= 2, "{asks}");
    let targets = gate.targets();
    assert_eq!(
        targets[..2],
        ["/prod?token=expired", "/prod?token=fresh"],
        "{targets:?}"
    );
}

#[test]
fn a_null_answer_is_a_failed_relay_function_and_the_worker_asks_again() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let relay = runtime.block_on(LocalRelay::start());
    let mut bridge = open_with_relay_function();

    let asks = serve_until_connected(&mut bridge, |asked| match asked {
        0 => Value::Null,
        _ => text(&relay.url()),
    });

    assert!(asks >= 2, "{asks}");
}

/// A `relay` ask the SDK does not answer within the bound (NOC-585): the
/// worker reports `relay_url_failed` and asks again, and the late answer
/// of the first ask is ignored. A shorter bound than 30 s here.
#[test]
fn an_unanswered_ask_runs_out_of_time_and_its_late_answer_is_ignored() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let relay = runtime.block_on(LocalRelay::start());
    let gate = runtime.block_on(Gate::start(relay.addr, "fresh"));
    let mut bridge = crate::relay::with_bound(Duration::from_millis(300), open_with_relay_function);
    let url = |token: &str| text(&format!("{}/prod?token={token}", gate.url()));

    let start = Instant::now();
    let mut first = None;
    let mut failed = false;
    'served: loop {
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "not connected in time"
        );
        let events = bridge.events().expect("an open database").take();
        for event in events.iter().map(|bytes| cbor::decode(bytes).unwrap()) {
            match field(&event, "event") {
                Some(Value::Text(kind)) if kind == "relay" => {
                    let request = field(&event, "request").expect("a request").clone();
                    let Some(late) = first.replace(request.clone()) else {
                        // The first ask stays unanswered for now.
                        continue;
                    };
                    for (request, token) in [(late, "late"), (request, "fresh")] {
                        let answer = map(&[("request", request), ("relay", url(token))]);
                        assert_eq!(ok(&mut bridge, "relay.answer", answer), Value::Null);
                    }
                }
                Some(Value::Text(kind)) if kind == "sync" => {
                    let status = field(&event, "status").expect("a status");
                    failed |= field(status, "error") == Some(&text("relay_url_failed"));
                    if field(status, "connection") == Some(&text("connected")) {
                        break 'served;
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(failed, "the status never named relay_url_failed");
    let targets = gate.targets();
    assert!(
        !targets.iter().any(|target| target.contains("late")),
        "{targets:?}"
    );
}
