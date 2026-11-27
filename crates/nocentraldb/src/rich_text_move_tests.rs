//! A rich-text field in a move (RFC 0016, "Its rich-text fields") and in
//! an ad-hoc copy (RFC 0013, "Moving and copying the object"). The member
//! that moves or copies the object re-creates the field's document in the
//! other space from its tree, as one rich-text update under a client id it
//! leases there: every node, mark and attribute arrives, and every item
//! there names that member (NOC-539; the owner's decision on NOC-543). A
//! field nobody wrote stays unset.
//!
//! Built only with the compiler cfg `ncdb_rich_text` (NOC-468).

use std::collections::HashMap;
use std::sync::Arc;

use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlOut, XmlTextPrelim};
use yrs::updates::decoder::Decode;
use yrs::{Any, ClientID, Doc, GetString, Number, Options, ReadTxn, Text, Transact, Update};

use crate::ad_hoc::AdHoc;
use crate::db::{Db, OpenOptions};
use crate::id::{ObjectId, SpaceId};
use crate::members::tests::now;
use crate::object::Object;
use crate::rich_text::{self, RichTextUpdate};
use crate::space::Space;
use crate::value::Value;

const LOCK: &str = include_str!("../tests/model/model.lock.json");

/// The test lock, with a rich-text `body` on tasks.
fn lock() -> String {
    let title = r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#;
    assert!(LOCK.contains(title));
    LOCK.replacen(
        title,
        &format!(r#"{title} "body": {{ "rule": 1, "since": 3, "type": "ncdb/richText" }},"#),
        1,
    )
}

fn db() -> Db {
    now(Db::open(OpenOptions::new(&lock()).in_memory())).expect("the lock opens")
}

/// A document as the core holds it.
fn load(document: &[u8]) -> Doc {
    let doc = Doc::new();
    if !document.is_empty() {
        let update = Update::decode_v1(document).unwrap();
        doc.transact_mut().apply_update(update).unwrap();
    }
    doc
}

/// The document's root fragment as XML, its nodes, attributes and marks,
/// each node's attributes sorted by name: Yrs writes them in no fixed
/// order.
fn xml(document: &[u8]) -> String {
    let doc = load(document);
    let root = doc.get_or_insert_xml_fragment("default");
    let txn = doc.transact();
    let mut out = String::new();
    for child in root.children(&txn) {
        node(&txn, child, &mut out);
    }
    out
}

fn node<T: ReadTxn>(txn: &T, node: XmlOut, out: &mut String) {
    match node {
        XmlOut::Element(element) => {
            let mut attributes: Vec<(String, String)> = element
                .attributes(txn)
                .map(|(name, value)| (name.to_owned(), value.to_string(txn)))
                .collect();
            attributes.sort();
            out.push_str(&format!("<{} {attributes:?}>", element.tag()));
            for child in element.children(txn) {
                self::node(txn, child, out);
            }
            out.push_str(&format!("</{}>", element.tag()));
        }
        XmlOut::Text(text) => out.push_str(&text.get_string(txn)),
        XmlOut::Fragment(fragment) => {
            for child in fragment.children(txn) {
                self::node(txn, child, out);
            }
        }
    }
}

/// The attribute `level` of the document's second node, as Yjs holds it:
/// a number stays a number.
fn level(document: &[u8]) -> Option<Any> {
    let doc = load(document);
    let root = doc.get_or_insert_xml_fragment("default");
    let txn = doc.transact();
    let callout = root.get(&txn, 1)?.into_xml_element()?;
    match callout.get_attribute(&txn, "level")? {
        yrs::Out::Any(any) => Some(any),
        _ => None,
    }
}

/// The client ids whose items `document` holds.
fn clients(document: &[u8]) -> Vec<u64> {
    let doc = load(document);
    let vector = doc.transact().state_vector();
    vector.iter().map(|(client, _)| client.get()).collect()
}

fn body(db: &Db, space: SpaceId, id: ObjectId) -> Vec<u8> {
    rich_text::document(&db.lock(), space, "tasks", id, "body").unwrap()
}

/// A task in `space`; when `written`, its body holds a paragraph with a
/// bold word and a node no editor schema of the test knows, with an
/// attribute. Returns the task and the client id the body was written
/// under.
fn task(db: &Db, space: &Space, written: bool) -> (ObjectId, Option<u64>) {
    let id = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("Share me"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    if !written {
        return (id, None);
    }
    let opened = rich_text::open(&mut db.lock(), space.id(), "tasks", id, "body").unwrap();
    let lease = opened.binding.lease;
    let doc = Doc::with_options(Options {
        client_id: ClientID::new(lease.client),
        ..Options::default()
    });
    let root = doc.get_or_insert_xml_fragment("default");
    {
        let mut txn = doc.transact_mut();
        let paragraph = root.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        let text = paragraph.insert(&mut txn, 0, XmlTextPrelim::new("Minutes, "));
        let bold: HashMap<Arc<str>, Any> = HashMap::from([("bold".into(), Any::Bool(true))]);
        text.insert_with_attributes(&mut txn, 9, "draft", bold);
        let callout = root.insert(&mut txn, 1, XmlElementPrelim::empty("callout"));
        callout.insert_attribute(&mut txn, "tone", "warning");
        callout.insert_attribute(&mut txn, "level", Any::Number(Number::Float(2.0)));
        callout.insert(&mut txn, 0, XmlTextPrelim::new("Not for the client"));
    }
    let update = RichTextUpdate {
        update: doc
            .transact()
            .encode_state_as_update_v1(&Default::default()),
        client: lease.client,
        nonce: lease.nonce,
    };
    now(space.update("tasks", id, [("body", update.value())])).unwrap();
    let held = xml(&body(db, space.id(), id));
    assert!(held.contains("draft") && held.contains("callout") && held.contains("warning"));
    (id, Some(lease.client))
}

fn read(space: &Space, id: ObjectId) -> Object {
    now(space.get("tasks", id)).unwrap().unwrap()
}

#[test]
fn a_move_re_creates_a_written_rich_text_document_under_a_client_id_of_the_mover() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (t, written_under) = task(&db, &personal, true);
    let before = body(&db, personal.id(), t);

    now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap();
    let moved = body(&db, shared.id(), t);
    assert_eq!(xml(&moved), xml(&before), "every node, mark and attribute");
    assert_eq!(
        level(&moved),
        Some(Any::Number(Number::Float(2.0))),
        "a number stays a number"
    );
    let under = clients(&moved);
    assert_eq!(under.len(), 1, "one client id writes the whole document");
    assert_ne!(Some(under[0]), written_under, "a client id leased anew");
}

#[test]
fn an_object_whose_rich_text_field_nobody_wrote_moves() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (t, _) = task(&db, &personal, false);

    now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap();
    assert_eq!(read(&personal, t).moved_to(), Some(shared.id()));
    assert_eq!(
        read(&shared, t).get::<&str>("title").unwrap(),
        Some("Share me")
    );
    assert!(body(&db, shared.id(), t).is_empty(), "the body stays unset");
}

#[test]
fn an_ad_hoc_copy_re_creates_a_written_rich_text_document_under_a_client_id_of_the_parent() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let (t, written_under) = task(&db, &personal, true);
    let before = body(&db, personal.id(), t);

    let created = now(db.spaces().create_ad_hoc(AdHoc {
        copy: vec![read(&personal, t)],
        ..AdHoc::default()
    }))
    .unwrap();
    let (_, copy) = created.copies[0];
    let copied = body(&db, created.space.id(), copy);
    assert_eq!(xml(&copied), xml(&before), "every node, mark and attribute");
    assert_eq!(
        level(&copied),
        Some(Any::Number(Number::Float(2.0))),
        "a number stays a number"
    );
    let under = clients(&copied);
    assert_eq!(under.len(), 1, "one client id writes the whole document");
    assert_ne!(Some(under[0]), written_under, "a client id leased anew");
}

#[test]
fn an_object_whose_rich_text_field_nobody_wrote_is_copied_into_an_ad_hoc_space() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let (t, _) = task(&db, &personal, false);

    let created = now(db.spaces().create_ad_hoc(AdHoc {
        copy: vec![read(&personal, t)],
        ..AdHoc::default()
    }))
    .unwrap();
    let (_, copy) = created.copies[0];
    let copied = read(&created.space, copy);
    assert_eq!(copied.get::<&str>("title").unwrap(), Some("Share me"));
    assert!(body(&db, created.space.id(), copy).is_empty());
}

#[test]
fn a_move_back_of_a_written_rich_text_document_is_refused_until_it_writes_only_the_difference() {
    // The old space's log already holds the document: re-creating it there
    // would show its text twice. Until a move back writes only the
    // difference (NOC-560; NOC-581), the move is refused, naming the object
    // and the field, and writes nothing.
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (t, _) = task(&db, &personal, true);
    now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap();
    let before = body(&db, shared.id(), t);

    let refused = now(shared.move_objects(&[read(&shared, t)], &personal)).unwrap_err();
    assert_eq!(refused.code(), crate::ErrorCode::NotAllowed);
    let message = refused.to_string();
    assert!(
        message.contains("body") && message.contains(&t.to_string()),
        "{message}"
    );
    assert!(
        !message.contains("draft"),
        "no content in an error: {message}"
    );
    assert_eq!(
        read(&personal, t).moved_to(),
        Some(shared.id()),
        "still a trace"
    );
    assert_eq!(read(&shared, t).moved_to(), None, "still live");
    assert_eq!(body(&db, shared.id(), t), before);
}

#[test]
fn a_move_back_of_an_object_whose_rich_text_field_nobody_wrote_is_not_refused() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (t, _) = task(&db, &personal, false);
    now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap();

    now(shared.move_objects(&[read(&shared, t)], &personal)).unwrap();
}
