//! A rich-text field as the Rust SDK reads it (RFC 0013, "The Rust SDK";
//! NOC-463): `object.get::<RichText>("body")`, with the document as
//! Markdown, as plain text, as the tree, and the references its mentions
//! hold. The document is written here as the editor binding writes it,
//! under a leased client id.

use std::collections::BTreeMap;

use ncdb_plugin::Data;
use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlTextPrelim};
use yrs::{Any, ClientID, Doc, OffsetKind, Options, Text, Transact};

use crate::error::ErrorCode;
use crate::id::{ObjectId, SpaceId};
use crate::rich_text::{self, RichTextUpdate};
use crate::space::Space;
use crate::value::{Reference, Value};
use crate::{Db, OpenOptions, RichText};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// The test lock, with a rich-text `body` on tasks.
pub(crate) fn lock() -> String {
    let title = r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#;
    LOCK.replacen(
        title,
        &format!(r#"{title} "body": {{ "rule": 1, "since": 3, "type": "ncdb/richText" }},"#),
        1,
    )
}

/// Polls `future`, which must not wait.
pub(crate) fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// A database with the rich-text lock, and a space of its own.
pub(crate) fn space() -> (Db, Space) {
    let db = now(Db::open(OpenOptions::new(&lock()).in_memory())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    (db, space)
}

const SPACE: &str = "0123456789abcdef0123456789abcdef";
const OBJECT: &str = "fedcba9876543210fedcba9876543210";

/// Writes a heading and a paragraph with a mention and bold text into the
/// body of `task`, as the editor binding does.
fn write_document(db: &Db, space: &Space, task: ObjectId) {
    let opened = rich_text::open(&mut db.lock(), space.id(), "tasks", task, "body").unwrap();
    let lease = opened.binding.lease;
    let doc = Doc::with_options(Options {
        client_id: ClientID::new(lease.client),
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    });
    let root = doc.get_or_insert_xml_fragment("default");
    let update = {
        let mut txn = doc.transact_mut();
        let heading = root.insert(&mut txn, 0, XmlElementPrelim::empty("heading"));
        heading.insert_attribute(&mut txn, "level", Any::from(2_i64));
        heading.insert(&mut txn, 0, XmlTextPrelim::new("Plan"));
        let paragraph = root.insert(&mut txn, 1, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new("Ask "));
        let mention = paragraph.insert(&mut txn, 1, XmlElementPrelim::empty("mention"));
        mention.insert_attribute(&mut txn, "space", Any::from(SPACE));
        mention.insert_attribute(&mut txn, "id", Any::from(OBJECT));
        let after = paragraph.insert(&mut txn, 2, XmlTextPrelim::new(" about "));
        let bold = std::collections::HashMap::from([(
            std::sync::Arc::<str>::from("bold"),
            Any::from(std::collections::HashMap::<String, Any>::new()),
        )]);
        after.insert_with_attributes(&mut txn, 7, "it", bold);
        txn.encode_update_v1()
    };
    let update = RichTextUpdate {
        update,
        client: lease.client,
        nonce: lease.nonce,
    };
    now(space.update("tasks", task, [("body", update.value())])).unwrap();
    rich_text::release(&mut db.lock(), &opened.binding).unwrap();
}

fn text(value: &str) -> Data {
    Data::Text(value.to_owned())
}

fn node(kind: &str, rest: Vec<(&str, Data)>) -> Data {
    let mut entries = BTreeMap::from([("type".to_owned(), text(kind))]);
    entries.extend(rest.into_iter().map(|(k, v)| (k.to_owned(), v)));
    Data::Map(entries)
}

fn read(space: &Space, task: ObjectId) -> RichText {
    now(space.get("tasks", task))
        .unwrap()
        .expect("the task")
        .get::<RichText>("body")
        .unwrap()
        .expect("the body holds a document")
}

#[test]
fn a_rich_text_field_reads_as_markdown_plain_text_tree_and_mentions() {
    let (db, space) = space();
    let task = now(space.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    write_document(&db, &space, task);

    let body = read(&space, task);

    assert_eq!(
        body.markdown(),
        format!("## Plan\n\nAsk [@](ncdb:{SPACE}/{OBJECT}) about **it**")
    );
    assert_eq!(body.plain_text(), "Plan\nAsk @ about it");
    let expected = node(
        "doc",
        vec![(
            "content",
            Data::Array(vec![
                node(
                    "heading",
                    vec![
                        (
                            "attrs",
                            Data::Map(BTreeMap::from([("level".to_owned(), Data::Integer(2))])),
                        ),
                        (
                            "content",
                            Data::Array(vec![node("text", vec![("text", text("Plan"))])]),
                        ),
                    ],
                ),
                node(
                    "paragraph",
                    vec![(
                        "content",
                        Data::Array(vec![
                            node("text", vec![("text", text("Ask "))]),
                            node(
                                "mention",
                                vec![(
                                    "attrs",
                                    Data::Map(BTreeMap::from([
                                        ("id".to_owned(), text(OBJECT)),
                                        ("space".to_owned(), text(SPACE)),
                                    ])),
                                )],
                            ),
                            node("text", vec![("text", text(" about "))]),
                            node(
                                "text",
                                vec![
                                    ("text", text("it")),
                                    ("marks", Data::Array(vec![node("bold", vec![])])),
                                ],
                            ),
                        ]),
                    )],
                ),
            ]),
        )],
    );
    assert_eq!(body.tree(), &expected);
    let target = Reference::new(
        SpaceId(ncdb_core::ids::SpaceId([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab,
            0xcd, 0xef,
        ])),
        ObjectId(ncdb_core::ids::ObjectId([
            0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32, 0x10, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54,
            0x32, 0x10,
        ])),
    );
    assert_eq!(body.mentions(), &[target]);
}

#[test]
fn a_rich_text_field_nothing_was_written_to_reads_unset() {
    let (_db, space) = space();
    let task = now(space.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let read = now(space.get("tasks", task)).unwrap().expect("the task");
    assert_eq!(read.get::<RichText>("body").unwrap(), None);
}

#[test]
fn a_rich_text_field_read_as_another_kind_and_another_field_read_as_rich_text_are_refused() {
    let (db, space) = space();
    let task = now(space.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    write_document(&db, &space, task);
    let read = now(space.get("tasks", task)).unwrap().expect("the task");
    assert_eq!(
        read.get::<&str>("body").unwrap_err().code(),
        ErrorCode::InvalidValue
    );
    assert_eq!(
        read.get::<RichText>("title").unwrap_err().code(),
        ErrorCode::InvalidValue
    );
}

#[test]
fn a_rich_text_reads_redacted_in_debug_output() {
    let (db, space) = space();
    let task = now(space.create("tasks", [("title", Value::from("a"))]))
        .unwrap()
        .ids[0];
    write_document(&db, &space, task);
    let shown = format!("{:?}", read(&space, task));
    assert!(
        !shown.contains("Plan") && !shown.contains(OBJECT),
        "{shown}"
    );
}
