//! Following the space list and leaving a space through the bridge
//! (NOC-315, slice 1; RFC 0009, "Spaces").

use ncdb_core::cbor::Value;

use super::{Bridge, as_text, code, event, field, map, ok, opened, text};

/// The spaces of a `spaces.list` answer or a `spaces` event.
fn entries(list: &Value) -> &[Value] {
    match list {
        Value::Array(entries) => entries,
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn spaces_list_names_each_spaces_model() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let list = ok(&mut bridge, "spaces.list", map(&[]));
    let [entry] = entries(&list) else {
        panic!("one space: {list:?}")
    };
    assert_eq!(field(entry, "id"), Some(&space));
    assert_eq!(field(entry, "role"), Some(&text("admin")));
    assert_eq!(field(entry, "model"), Some(&text("ours")));
}

#[test]
fn spaces_follow_answers_the_list_and_then_a_spaces_event_on_each_change() {
    let mut bridge = opened();
    let events = bridge.events().expect("events once open");
    let first = ok(&mut bridge, "spaces.follow", map(&[]));
    assert!(entries(&first).is_empty(), "{first:?}");
    events.take();

    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let message = event(&events, "spaces").expect("a spaces event");
    let list = field(&message, "spaces").expect("the list");
    let [entry] = entries(list) else {
        panic!("one space: {list:?}")
    };
    assert_eq!(field(entry, "id"), Some(&space));
    assert_eq!(field(entry, "model"), Some(&text("ours")));

    // A write changes no space's entry, so no spaces event.
    events.take();
    ok(
        &mut bridge,
        "write",
        map(&[("space", space.clone()), ("ops", Value::Array(vec![]))]),
    );
    assert!(event(&events, "spaces").is_none(), "unchanged");
    ok(&mut bridge, "close", map(&[]));
}

#[test]
fn without_spaces_follow_no_spaces_event_is_sent() {
    let mut bridge = opened();
    let events = bridge.events().expect("events once open");
    events.take();
    ok(&mut bridge, "spaces.create", map(&[]));
    assert!(event(&events, "spaces").is_none());
}

#[test]
fn the_last_admin_cannot_leave() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    assert_eq!(
        code(&mut bridge, "space.leave", map(&[("space", space)])),
        "lastAdmin"
    );
}

#[test]
fn a_member_leaves_once_another_admin_holds_the_space() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let mut other: Bridge = opened();
    let invitation = field(&ok(&mut other, "invitation", map(&[])), "text")
        .expect("a text")
        .clone();
    let fingerprint = field(&ok(&mut other, "invitation", map(&[])), "fingerprint")
        .expect("a fingerprint")
        .clone();
    ok(
        &mut bridge,
        "members.add",
        map(&[
            ("space", space.clone()),
            ("invitations", Value::Array(vec![invitation])),
            ("role", text("admin")),
            ("fingerprint", fingerprint),
        ]),
    );

    let left = ok(&mut bridge, "space.leave", map(&[("space", space.clone())]));
    let notice = field(&left, "notice").expect("a notice");
    assert_eq!(field(notice, "code"), Some(&text("spaceLeft")));
    assert!(
        as_text(field(notice, "text").expect("a text")).starts_with("This member keeps its copy")
    );

    let list = ok(&mut bridge, "spaces.list", map(&[]));
    let [entry] = entries(&list) else {
        panic!("one space: {list:?}")
    };
    assert_eq!(field(entry, "role"), Some(&Value::Null));
    assert_eq!(
        code(&mut bridge, "space.leave", map(&[("space", space)])),
        "notMember"
    );
}
