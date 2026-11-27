//! The bridge's calls for a rich-text editor binding (RFC 0013, "An editor:
//! the binding"; NOC-461): a lease of a client id with the field's state,
//! the part of the document the binding does not hold yet, the write of an
//! update under the lease, and the release with the clock it reached.

use ncdb_core::cbor::Value;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
    ClientID, Doc, GetString, Options, ReadTxn, StateVector, Transact, Update, XmlFragment,
    XmlTextPrelim,
};

use super::fields::{op, opened_with, write};
use super::{Bridge, LOCK, code, field, map, ok, text};

/// The bridge tests' lock, with a rich-text `body` on tasks.
fn lock() -> String {
    let title = r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#;
    assert!(LOCK.contains(title));
    LOCK.replacen(
        title,
        &format!(r#"{title} "body": {{ "rule": 1, "since": 3, "type": "ncdb/richText" }},"#),
        1,
    )
}

/// A bridge with a space and one task in it: the space and the task's id.
fn with_task() -> (Bridge, Value, Value) {
    let mut bridge = opened_with(&lock());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let ids = write(
        &mut bridge,
        &space,
        vec![op("create", "tasks", None, map(&[("title", text("Note"))]))],
    );
    (bridge, space, ids[0].clone())
}

fn open(bridge: &mut Bridge, space: &Value, id: &Value, field_name: &str) -> Value {
    ok(bridge, "richText.open", target(space, id, field_name))
}

fn target(space: &Value, id: &Value, field_name: &str) -> Value {
    map(&[
        ("space", space.clone()),
        ("collection", text("tasks")),
        ("id", id.clone()),
        ("field", text(field_name)),
    ])
}

fn unsigned(value: &Value, name: &str) -> u64 {
    match field(value, name) {
        Some(Value::Unsigned(n)) => *n,
        other => panic!("{name}: {other:?}"),
    }
}

fn bytes(value: &Value, name: &str) -> Vec<u8> {
    match field(value, name) {
        Some(Value::Bytes(b)) => b.clone(),
        other => panic!("{name}: {other:?}"),
    }
}

/// A Yjs document as an editor's binding holds it, under `client`.
fn doc(client: u64) -> Doc {
    Doc::with_options(Options {
        client_id: ClientID::new(client),
        ..Options::default()
    })
}

fn apply(doc: &Doc, update: &[u8]) {
    doc.transact_mut()
        .apply_update(Update::decode_v1(update).unwrap())
        .unwrap();
}

/// What `doc` holds of the root fragment, as XML.
fn xml(doc: &Doc) -> String {
    let root = doc.get_or_insert_xml_fragment("default");
    root.get_string(&doc.transact())
}

/// Inserts a paragraph of `words` at the end of `doc`'s root fragment;
/// the update that carries it.
fn type_paragraph(doc: &Doc, words: &str) -> Vec<u8> {
    let root = doc.get_or_insert_xml_fragment("default");
    let before = doc.transact().state_vector();
    {
        let mut txn = doc.transact_mut();
        let end = root.len(&txn);
        root.insert(&mut txn, end, XmlTextPrelim::new(words));
    }
    doc.transact().encode_state_as_update_v1(&before)
}

fn write_update(bridge: &mut Bridge, lease: u64, update: Vec<u8>) -> Value {
    let args = map(&[
        ("lease", Value::Unsigned(lease)),
        ("update", Value::Bytes(update)),
    ]);
    ok(bridge, "richText.write", args)
}

fn pull(bridge: &mut Bridge, lease: u64, state_vector: Vec<u8>) -> Vec<u8> {
    let answer = ok(
        bridge,
        "richText.pull",
        map(&[
            ("lease", Value::Unsigned(lease)),
            ("stateVector", Value::Bytes(state_vector)),
        ]),
    );
    match answer {
        Value::Bytes(b) => b,
        other => panic!("{other:?}"),
    }
}

#[test]
fn open_leases_a_client_id_and_gives_the_fields_state() {
    let (mut bridge, space, id) = with_task();
    let opened = open(&mut bridge, &space, &id, "body");
    assert_eq!(field(&opened, "writable"), Some(&Value::Bool(true)));
    assert_eq!(unsigned(&opened, "clock"), 0);
    let client = unsigned(&opened, "client");
    assert!(client < 1 << 53, "a client id a JavaScript number holds");
    let binding = doc(client);
    apply(&binding, &bytes(&opened, "state"));
    assert_eq!(xml(&binding), "");
}

#[test]
fn two_bindings_at_once_lease_two_client_ids() {
    let (mut bridge, space, id) = with_task();
    let one = open(&mut bridge, &space, &id, "body");
    let two = open(&mut bridge, &space, &id, "body");
    assert_ne!(unsigned(&one, "client"), unsigned(&two, "client"));
    assert_ne!(unsigned(&one, "lease"), unsigned(&two, "lease"));
}

#[test]
fn open_refuses_an_object_the_copy_does_not_hold_and_a_field_that_is_not_rich_text() {
    let (mut bridge, space, id) = with_task();
    let absent = text("0123456789abcdef0123456789abcdef");
    assert_eq!(
        code(
            &mut bridge,
            "richText.open",
            target(&space, &absent, "body")
        ),
        "notFound"
    );
    assert_eq!(
        code(&mut bridge, "richText.open", target(&space, &id, "title")),
        "invalidValue"
    );
}

#[test]
fn a_write_under_the_lease_folds_and_another_binding_pulls_it() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let theirs = open(&mut bridge, &space, &id, "body");
    let editor = doc(unsigned(&mine, "client"));
    let update = type_paragraph(&editor, "Hello");
    let written = write_update(&mut bridge, unsigned(&mine, "lease"), update);
    assert!(field(&written, "change").is_some(), "{written:?}");

    let other = doc(unsigned(&theirs, "client"));
    let sv = other.transact().state_vector().encode_v1();
    apply(&other, &pull(&mut bridge, unsigned(&theirs, "lease"), sv));
    assert_eq!(xml(&other), "Hello");
}

#[test]
fn a_pull_gives_only_what_the_binding_does_not_hold() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let lease = unsigned(&mine, "lease");
    let editor = doc(unsigned(&mine, "client"));
    let update = type_paragraph(&editor, "Hello");
    write_update(&mut bridge, lease, update);
    let held = editor.transact().state_vector().encode_v1();
    let nothing_new = pull(&mut bridge, lease, held);
    let fresh = doc(1);
    apply(&fresh, &nothing_new);
    assert_eq!(xml(&fresh), "");
    assert_eq!(fresh.transact().state_vector(), StateVector::default());
}

#[test]
fn an_update_under_another_client_id_is_refused_and_nothing_is_written() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let lease = unsigned(&mine, "lease");
    let stranger = doc(unsigned(&mine, "client") ^ 1);
    let update = type_paragraph(&stranger, "Not mine");
    let args = map(&[
        ("lease", Value::Unsigned(lease)),
        ("update", Value::Bytes(update)),
    ]);
    assert_eq!(code(&mut bridge, "richText.write", args), "invalidValue");
    let fresh = doc(1);
    apply(&fresh, &pull(&mut bridge, lease, Vec::from([0u8])));
    assert_eq!(xml(&fresh), "");
}

#[test]
fn bytes_that_are_no_update_are_refused() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let args = map(&[
        ("lease", Value::Unsigned(unsigned(&mine, "lease"))),
        ("update", Value::Bytes(vec![0xff, 0x01, 0x02])),
    ]);
    assert_eq!(code(&mut bridge, "richText.write", args), "invalidValue");
}

#[test]
fn an_update_that_only_deletes_is_written() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let lease = unsigned(&mine, "lease");
    let editor = doc(unsigned(&mine, "client"));
    write_update(&mut bridge, lease, type_paragraph(&editor, "Hello"));
    let root = editor.get_or_insert_xml_fragment("default");
    let before = editor.transact().state_vector();
    root.remove_range(&mut editor.transact_mut(), 0, 1);
    let deletion = editor.transact().encode_state_as_update_v1(&before);
    write_update(&mut bridge, lease, deletion);
    let fresh = doc(1);
    apply(&fresh, &pull(&mut bridge, lease, Vec::from([0u8])));
    assert_eq!(xml(&fresh), "");
}

#[test]
fn close_releases_the_client_id_with_the_clock_it_reached() {
    let (mut bridge, space, id) = with_task();
    let mine = open(&mut bridge, &space, &id, "body");
    let (lease, client) = (unsigned(&mine, "lease"), unsigned(&mine, "client"));
    let editor = doc(client);
    write_update(&mut bridge, lease, type_paragraph(&editor, "Hello"));
    let reached = editor.transact().state_vector().get(&ClientID::new(client));
    assert!(reached > 0);
    ok(
        &mut bridge,
        "richText.close",
        map(&[("lease", Value::Unsigned(lease))]),
    );
    let again = open(&mut bridge, &space, &id, "body");
    assert_eq!(unsigned(&again, "client"), client);
    assert_eq!(unsigned(&again, "clock"), u64::from(reached));
    // A closed lease takes no more calls.
    let args = map(&[
        ("lease", Value::Unsigned(lease)),
        ("stateVector", Value::Bytes(vec![0])),
    ]);
    assert_eq!(code(&mut bridge, "richText.pull", args), "invalidValue");
}
