//! A rich-text field in a move (RFC 0016) and in an ad-hoc copy (RFC
//! 0003, "Sub-agents get an ad-hoc space"). No decided RFC says yet how a
//! rich-text document travels to another space (NOC-543), so an object
//! whose rich-text field was written is refused with `NotAllowed`, naming
//! the object and the field, and nothing is written (NOC-539). An object
//! whose rich-text field nobody wrote moves and copies as before.
//!
//! Built only with the compiler cfg `ncdb_rich_text` (NOC-468).

use yrs::types::xml::{XmlFragment, XmlTextPrelim};
use yrs::{ClientID, Doc, Options, ReadTxn, Transact};

use crate::ad_hoc::AdHoc;
use crate::db::{Db, OpenOptions};
use crate::error::ErrorCode;
use crate::id::ObjectId;
use crate::members::tests::now;
use crate::object::Object;
use crate::rich_text::{self, RichTextUpdate};
use crate::space::Space;
use crate::value::Value;

const LOCK: &str = include_str!("../tests/model/model.lock.json");
/// The text the tests write into the body; no error may carry it.
const WORDS: &str = "Confidential minutes";

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

/// A task in `space`, its body written with a paragraph when `written`.
fn task(db: &Db, space: &Space, written: bool) -> ObjectId {
    let id = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("Share me"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    if written {
        let opened = rich_text::open(&mut db.lock(), space.id(), "tasks", id, "body").unwrap();
        let lease = opened.binding.lease;
        let doc = Doc::with_options(Options {
            client_id: ClientID::new(lease.client),
            ..Options::default()
        });
        let root = doc.get_or_insert_xml_fragment("default");
        root.insert(&mut doc.transact_mut(), 0, XmlTextPrelim::new(WORDS));
        let update = RichTextUpdate {
            update: doc
                .transact()
                .encode_state_as_update_v1(&Default::default()),
            client: lease.client,
            nonce: lease.nonce,
        };
        now(space.update("tasks", id, [("body", update.value())])).unwrap();
        let held = rich_text::document(&db.lock(), space.id(), "tasks", id, "body").unwrap();
        assert!(!held.is_empty(), "the body holds the paragraph");
    }
    id
}

fn read(space: &Space, id: ObjectId) -> Object {
    now(space.get("tasks", id)).unwrap().unwrap()
}

/// The refusal names the object and the field, and holds no content.
fn names_the_field(message: &str, id: ObjectId) {
    assert!(message.contains("body"), "{message}");
    assert!(message.contains(&id.to_string()), "{message}");
    assert!(!message.contains(WORDS), "{message}");
}

#[test]
fn an_object_with_a_written_rich_text_field_cannot_be_moved_yet() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let t = task(&db, &personal, true);

    let refused = now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    names_the_field(&refused.to_string(), t);
    // Nothing is written: the new space has no object, the old no trace.
    assert!(now(shared.get("tasks", t)).unwrap().is_none());
    assert_eq!(read(&personal, t).moved_to(), None);
}

#[test]
fn an_object_whose_rich_text_field_nobody_wrote_moves() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let t = task(&db, &personal, false);

    now(personal.move_objects(&[read(&personal, t)], &shared)).unwrap();
    assert_eq!(read(&personal, t).moved_to(), Some(shared.id()));
    assert_eq!(
        read(&shared, t).get::<&str>("title").unwrap(),
        Some("Share me")
    );
}

#[test]
fn an_object_with_a_written_rich_text_field_is_not_copied_into_an_ad_hoc_space_yet() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let t = task(&db, &personal, true);
    let before = db.lock().spaces.len();

    let refused = now(db.spaces().create_ad_hoc(AdHoc {
        copy: vec![read(&personal, t)],
        ..AdHoc::default()
    }))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    names_the_field(&refused.to_string(), t);
    // A refused call creates nothing: no ad-hoc space.
    assert_eq!(db.lock().spaces.len(), before);
}

#[test]
fn an_object_whose_rich_text_field_nobody_wrote_is_copied_into_an_ad_hoc_space() {
    let db = db();
    let personal = now(db.spaces().create()).unwrap();
    let t = task(&db, &personal, false);

    let created = now(db.spaces().create_ad_hoc(AdHoc {
        copy: vec![read(&personal, t)],
        ..AdHoc::default()
    }))
    .unwrap();
    let (_, copy) = created.copies[0];
    let copied = read(&created.space, copy);
    assert_eq!(copied.get::<&str>("title").unwrap(), Some("Share me"));
}
