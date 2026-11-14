//! The durability of the copy and the view's report through the bridge
//! (NOC-315, slice 3; RFC 0006, "What the SDK reports"; RFC 0009, "Sync and
//! status"; the shapes the owner decided on NOC-317, Carsten, 2026-11-14).

use ncdb_core::cbor::Value;

use super::{Bridge, LOCK, event, field, map, ok, text};

/// A database in memory, and what `open` answered.
fn opened() -> (Bridge, Value) {
    let mut bridge = Bridge::new();
    let answer = ok(
        &mut bridge,
        "open",
        map(&[("lock", text(LOCK)), ("inMemory", Value::Bool(true))]),
    );
    (bridge, answer)
}

/// The number of this member's unconfirmed changes `durability` answers.
fn unconfirmed(durability: &Value) -> u64 {
    match field(durability, "unconfirmed") {
        Some(Value::Unsigned(count)) => *count,
        other => panic!("a count: {other:?}"),
    }
}

/// A new space; its genesis is this member's change too.
fn space(bridge: &mut Bridge) -> Value {
    ok(bridge, "spaces.create", map(&[]))
}

fn write_one(bridge: &mut Bridge, space: Value) {
    ok(
        bridge,
        "write",
        map(&[
            ("space", space),
            (
                "ops",
                Value::Array(vec![map(&[
                    ("op", text("create")),
                    ("collection", text("tasks")),
                    ("values", map(&[("title", text("Ship"))])),
                ])]),
            ),
        ]),
    );
}

#[test]
fn open_answers_the_durability_and_the_view() {
    let (_bridge, answer) = opened();
    let durability = field(&answer, "durability").expect("the durability");
    assert_eq!(field(durability, "storage"), Some(&text("temporary")));
    assert_eq!(field(durability, "unconfirmed"), Some(&Value::Unsigned(0)));
    assert_eq!(field(durability, "oldestUnconfirmedAt"), Some(&Value::Null));
    assert_eq!(field(durability, "atRisk"), Some(&Value::Bool(false)));
    let view = field(&answer, "view").expect("the view");
    assert_eq!(field(view, "rebuild"), Some(&Value::Null));
}

#[test]
fn a_write_without_a_relay_is_unconfirmed_and_at_risk_in_memory() {
    let (mut bridge, _) = opened();
    let space = space(&mut bridge);
    let before = unconfirmed(&ok(&mut bridge, "durability", map(&[])));
    write_one(&mut bridge, space);
    let durability = ok(&mut bridge, "durability", map(&[]));
    assert_eq!(unconfirmed(&durability), before + 1);
    let Some(Value::Unsigned(at)) = field(&durability, "oldestUnconfirmedAt") else {
        panic!("milliseconds since the epoch: {durability:?}")
    };
    assert!(*at > 1_700_000_000_000, "{at}");
    assert_eq!(field(&durability, "atRisk"), Some(&Value::Bool(true)));
}

#[test]
fn a_write_yields_a_durability_event() {
    let (mut bridge, _) = opened();
    let events = bridge.events().expect("events once open");
    let space = space(&mut bridge);
    let before = unconfirmed(&ok(&mut bridge, "durability", map(&[])));
    events.take();
    write_one(&mut bridge, space);
    let message = event(&events, "durability").expect("a durability event");
    let durability = field(&message, "durability").expect("the durability");
    assert_eq!(unconfirmed(durability), before + 1);
    // Nothing changed since: no second event.
    assert!(event(&events, "durability").is_none());
}

#[test]
fn view_answers_the_views_report() {
    let (mut bridge, _) = opened();
    let view = ok(&mut bridge, "view", map(&[]));
    assert_eq!(field(&view, "rebuild"), Some(&Value::Null));
}
