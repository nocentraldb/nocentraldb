//! A rich-text field read and first written through the bridge (RFC 0013,
//! "Reading" and "The first content"; NOC-589, NOC-578): every object read
//! gives the field's Markdown, a text, and a field nothing was written to
//! reads unset, `null`, as the Rust SDK reads it; `{"$markdown": text}`
//! writes a first document through the core's read-back, the path
//! `RichText::from_markdown` takes, so the shared table of cases reads the
//! same through the bridge as in Rust.

use ncdb_core::cbor::Value;
use serde_json::Value as Json;
use yrs::types::xml::{XmlElementPrelim, XmlFragment, XmlTextPrelim};
use yrs::{ReadTxn, StateVector, Transact};

use super::fields::{fields, op, write};
use super::rich_text::{doc, lock, open, unsigned, with_task, write_update};
use super::{Bridge, as_text, code, field, map, now, ok, text};
use crate::RichText;
use crate::id::{ObjectId, SpaceId};

const TABLE: &str =
    include_str!("../../../../ncdb-core/tests/vectors/rich-text/from-markdown.json");

fn table(part: &str) -> Vec<Json> {
    let table: Json = serde_json::from_str(TABLE).expect("the table is JSON");
    table[part].as_array().expect("a list of cases").clone()
}

/// The `body` of the task `id`, as `get` answers it.
fn body(bridge: &mut Bridge, space: &Value, id: &Value) -> Value {
    field(&fields(bridge, space, "tasks", id), "body")
        .expect("every field of the lock is answered")
        .clone()
}

/// A first document from `markdown`, as the TypeScript SDK sends it.
fn markdown(markdown: &str) -> Value {
    map(&[("$markdown", text(markdown))])
}

/// One paragraph of `words`, written by a binding under its lease.
fn type_into(bridge: &mut Bridge, space: &Value, id: &Value, words: &str) {
    let opened = open(bridge, space, id, "body");
    let editor = doc(unsigned(&opened, "client"));
    let root = editor.get_or_insert_xml_fragment("default");
    let update = {
        let mut txn = editor.transact_mut();
        let paragraph = root.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new(words));
        txn.encode_state_as_update_v1(&StateVector::default())
    };
    write_update(bridge, unsigned(&opened, "lease"), update);
}

#[test]
fn a_field_nothing_was_written_to_reads_null() {
    let (mut bridge, space, id) = with_task();
    assert_eq!(body(&mut bridge, &space, &id), Value::Null);
}

#[test]
fn a_field_a_binding_wrote_reads_as_its_markdown() {
    let (mut bridge, space, id) = with_task();
    type_into(&mut bridge, &space, &id, "Typed *here*");
    assert_eq!(body(&mut bridge, &space, &id), text("Typed \\*here\\*"));
}

#[test]
fn a_field_the_rust_sdk_wrote_reads_as_its_markdown() {
    let (mut bridge, space, id) = with_task();
    {
        let db = bridge.db.as_ref().expect("a database is open");
        let handle = now(db.spaces().get(as_text(&space).parse::<SpaceId>().unwrap())).unwrap();
        let object: ObjectId = as_text(&id).parse().unwrap();
        now(handle.update(
            "tasks",
            object,
            [("body", RichText::from_markdown("## Plan\n\nShip **it**."))],
        ))
        .unwrap();
    }
    assert_eq!(
        body(&mut bridge, &space, &id),
        text("## Plan\n\nShip **it**.")
    );
}

#[test]
fn a_query_reads_the_field_as_its_markdown() {
    let mut bridge = super::fields::opened_with(&lock());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "tasks",
            None,
            map(&[
                ("title", text("Note")),
                ("priority", Value::Unsigned(1)),
                ("body", markdown("- one\n- two")),
            ]),
        )],
    );
    let page = ok(
        &mut bridge,
        "find",
        map(&[(
            "query",
            map(&[
                ("collection", text("tasks")),
                ("index", text("byPriority")),
                ("take", Value::Unsigned(1)),
            ]),
        )]),
    );
    let Some(Value::Array(items)) = field(&page, "items") else {
        panic!("{page:?}")
    };
    let read = field(field(&items[0], "fields").unwrap(), "body").cloned();
    assert_eq!(read, Some(text("- one\n- two")));
}

#[test]
fn every_case_of_the_table_is_written_on_create_and_reads_as_its_markdown() {
    let mut bridge = super::fields::opened_with(&lock());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    for case in table("cases") {
        let name = case["name"].as_str().expect("a name");
        let written = case["markdown"].as_str().expect("Markdown");
        let ids = write(
            &mut bridge,
            &space,
            vec![op(
                "create",
                "tasks",
                None,
                map(&[("body", markdown(written))]),
            )],
        );
        let reads = case["reads"].as_str().expect("reads");
        assert_eq!(body(&mut bridge, &space, &ids[0]), text(reads), "{name}");
    }
}

#[test]
fn every_refused_case_of_the_table_is_refused_with_invalid_value() {
    let mut bridge = super::fields::opened_with(&lock());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    for case in table("refused") {
        let name = case["name"].as_str().expect("a name");
        let written = case["markdown"].as_str().expect("Markdown");
        let args = map(&[
            ("space", space.clone()),
            (
                "ops",
                Value::Array(vec![op(
                    "create",
                    "tasks",
                    None,
                    map(&[("title", text("a")), ("body", markdown(written))]),
                )]),
            ),
        ]);
        assert_eq!(code(&mut bridge, "write", args), "invalidValue", "{name}");
    }
}

#[test]
fn an_update_writes_a_first_document_and_a_second_is_refused_with_rich_text_not_empty() {
    let (mut bridge, space, id) = with_task();
    write(
        &mut bridge,
        &space,
        vec![op(
            "update",
            "tasks",
            Some(&id),
            map(&[("body", markdown("First."))]),
        )],
    );
    assert_eq!(body(&mut bridge, &space, &id), text("First."));
    let args = map(&[
        ("space", space.clone()),
        (
            "ops",
            Value::Array(vec![op(
                "update",
                "tasks",
                Some(&id),
                map(&[("body", markdown("Second."))]),
            )]),
        ),
    ]);
    assert_eq!(code(&mut bridge, "write", args), "richTextNotEmpty");
    assert_eq!(body(&mut bridge, &space, &id), text("First."));
}

#[test]
fn markdown_is_refused_for_a_field_that_is_not_rich_text() {
    let (mut bridge, space, id) = with_task();
    let args = map(&[
        ("space", space.clone()),
        (
            "ops",
            Value::Array(vec![op(
                "update",
                "tasks",
                Some(&id),
                map(&[("title", markdown("Plan"))]),
            )]),
        ),
    ]);
    assert_eq!(code(&mut bridge, "write", args), "invalidValue");
}

/// The document a binding opens holds what `$markdown` wrote, so an
/// editor edits on from the first document.
#[test]
fn a_binding_opens_on_the_first_document() {
    let (mut bridge, space, id) = with_task();
    write(
        &mut bridge,
        &space,
        vec![op(
            "update",
            "tasks",
            Some(&id),
            map(&[("body", markdown("Hello"))]),
        )],
    );
    let opened = open(&mut bridge, &space, &id, "body");
    let Some(Value::Bytes(state)) = field(&opened, "state") else {
        panic!("{opened:?}")
    };
    let editor = doc(1);
    editor
        .transact_mut()
        .apply_update(yrs::updates::decoder::Decode::decode_v1(state).unwrap())
        .unwrap();
    let root = editor.get_or_insert_xml_fragment("default");
    let txn = editor.transact();
    assert_eq!(
        yrs::GetString::get_string(&root, &txn),
        "<paragraph>Hello</paragraph>"
    );
    assert!(!txn.state_vector().is_empty());
}
