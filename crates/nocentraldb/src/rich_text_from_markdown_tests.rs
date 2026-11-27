//! A first document written from Markdown (RFC 0013, "The first content"
//! and "The Rust SDK"; NOC-463): `RichText::from_markdown` writes a whole
//! document, and only into a field that holds no content, on `create` or
//! on a field that holds none in the copy; on a field with content it is
//! refused with `RichTextNotEmpty`. The Markdown is read by the core's
//! read-back, the table of cases shared with the core and the TypeScript
//! SDK.

use ncdb_plugin::Data;
use serde_json::Value as Json;
use yrs::types::xml::XmlFragment;
use yrs::updates::decoder::Decode;
use yrs::{ClientID, Doc, OffsetKind, Options, Transact, Update};

use crate::RichText;
use crate::error::ErrorCode;
use crate::id::ObjectId;
use crate::rich_text::{self, RichTextUpdate};
use crate::rich_text_field_tests::{now, space};
use crate::space::Space;
use crate::value::Value;

const TABLE: &str = include_str!("../../ncdb-core/tests/vectors/rich-text/from-markdown.json");

fn table(part: &str) -> Vec<Json> {
    let table: Json = serde_json::from_str(TABLE).expect("the table is JSON");
    table[part].as_array().expect("a list of cases").clone()
}

/// A tree of the table as the `Data` the SDK reads.
fn data(json: &Json) -> Data {
    match json {
        Json::Null => Data::Null,
        Json::Bool(b) => Data::Bool(*b),
        Json::Number(n) => match n.as_i64() {
            Some(n) => Data::Integer(n.into()),
            None => Data::Float(n.as_f64().expect("a number")),
        },
        Json::String(text) => Data::Text(text.clone()),
        Json::Array(items) => Data::Array(items.iter().map(data).collect()),
        Json::Object(entries) => {
            Data::Map(entries.iter().map(|(k, v)| (k.clone(), data(v))).collect())
        }
    }
}

fn body(space: &Space, task: ObjectId) -> Option<RichText> {
    now(space.get("tasks", task))
        .unwrap()
        .expect("the task")
        .get::<RichText>("body")
        .unwrap()
}

#[test]
fn every_case_of_the_table_is_written_on_create_and_reads_as_its_document() {
    let (_db, space) = space();
    for case in table("cases") {
        let name = case["name"].as_str().expect("a name");
        let markdown = case["markdown"].as_str().expect("Markdown");
        let task = now(space.create("tasks", [("body", RichText::from_markdown(markdown))]))
            .unwrap_or_else(|error| panic!("{name}: {error:?}"))
            .ids[0];
        let read = body(&space, task).unwrap_or_else(|| panic!("{name}: unset"));
        assert_eq!(read.tree(), &data(&case["tree"]), "{name}");
        assert_eq!(
            read.markdown(),
            case["reads"].as_str().expect("reads"),
            "{name}"
        );
    }
}

#[test]
fn every_refused_case_of_the_table_is_refused_with_invalid_value() {
    let (_db, space) = space();
    for case in table("refused") {
        let name = case["name"].as_str().expect("a name");
        let markdown = case["markdown"].as_str().expect("Markdown");
        let refused = now(space.create(
            "tasks",
            [
                ("title", Value::from("a")),
                ("body", RichText::from_markdown(markdown)),
            ],
        ));
        assert_eq!(
            refused.unwrap_err().code(),
            ErrorCode::InvalidValue,
            "{name}"
        );
    }
}

#[test]
fn an_update_writes_a_first_document_into_a_field_that_holds_none() {
    let (_db, space) = space();
    let task = now(space.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    now(space.update(
        "tasks",
        task,
        [("body", RichText::from_markdown("## Plan\n\nShip it."))],
    ))
    .unwrap();
    let read = body(&space, task).expect("a document");
    assert_eq!(read.markdown(), "## Plan\n\nShip it.");
}

#[test]
fn a_field_with_content_is_refused_with_rich_text_not_empty() {
    let (_db, space) = space();
    let task = now(space.create("tasks", [("body", RichText::from_markdown("First."))]))
        .unwrap()
        .ids[0];
    let refused = now(space.update(
        "tasks",
        task,
        [("body", RichText::from_markdown("Second."))],
    ));
    assert_eq!(refused.unwrap_err().code(), ErrorCode::RichTextNotEmpty);
    assert_eq!(body(&space, task).expect("a document").markdown(), "First.");
}

#[test]
fn a_field_whose_every_block_was_deleted_holds_no_content() {
    let (db, space) = space();
    let task = now(space.create("tasks", [("body", RichText::from_markdown("Gone."))]))
        .unwrap()
        .ids[0];
    // A binding deletes the document's one block.
    let opened = rich_text::open(&mut db.lock(), space.id(), "tasks", task, "body").unwrap();
    let doc = Doc::with_options(Options {
        client_id: ClientID::new(opened.binding.lease.client),
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    });
    let root = doc.get_or_insert_xml_fragment("default");
    doc.transact_mut()
        .apply_update(Update::decode_v1(&opened.document).unwrap())
        .unwrap();
    let update = {
        let mut txn = doc.transact_mut();
        root.remove_range(&mut txn, 0, 1);
        txn.encode_update_v1()
    };
    let update = RichTextUpdate {
        update,
        client: opened.binding.lease.client,
        nonce: opened.binding.lease.nonce,
    };
    now(space.update("tasks", task, [("body", update.value())])).unwrap();
    rich_text::release(&mut db.lock(), &opened.binding).unwrap();
    assert_eq!(body(&space, task).expect("a document").markdown(), "");

    now(space.update("tasks", task, [("body", RichText::from_markdown("Back."))])).unwrap();
    assert_eq!(body(&space, task).expect("a document").markdown(), "Back.");
}
