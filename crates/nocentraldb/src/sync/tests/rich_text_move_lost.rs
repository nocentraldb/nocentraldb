//! A rich-text field whose document holds a lost operation, in a move
//! (RFC 0016, "Its rich-text fields"; NOC-539). A simulated admin deletes
//! a paragraph the member types into at the same time, so the member's
//! typing is lost. The move resolves that as it resolves every field's
//! conflicts: the lost typing stays in the old space's log, and the move's
//! result lists the field among the fields that held a conflict.

use ncdb_core::cbor::{Key, Value as Cbor};
use ncdb_core::change::{Change, Operation};
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role as CoreRole;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::rich_text::{UPDATE, client_id};
use yrs::types::xml::{XmlElementPrelim, XmlFragment, XmlTextPrelim};
use yrs::updates::decoder::Decode;
use yrs::{ClientID, Doc, Options, ReadTxn, StateVector, Text, Transact, Update};

use super::support::{LOCK, Who, now, space};
use crate::db::{Db, OpenOptions, SpaceState};
use crate::id::{ObjectId, SpaceId};
use crate::rich_text::{self, RichTextUpdate};

const ADMIN: Who = 0;
const ME: Who = 1;
const TASK: CoreObjectId = CoreObjectId([0x42; 16]);
const NONCE: [u8; 8] = [9; 8];

/// The test lock, with a rich-text `body` on tasks.
fn lock() -> String {
    let title = r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#;
    LOCK.replacen(
        title,
        &format!(r#"{title} "body": {{ "rule": 1, "since": 3, "type": "ncdb/richText" }},"#),
        1,
    )
}

fn receive(db: &Db, changes: &[&Change]) {
    let mut inner = db.lock();
    let inner = &mut *inner;
    for change in changes {
        let space = change.header().space;
        let state = inner
            .spaces
            .entry(space)
            .or_insert_with(|| SpaceState::new(space, inner.member.key()));
        inner
            .view
            .receive(change.bytes(), &mut state.checks)
            .expect("the change folds");
    }
}

/// Operation 5 on the task's body: `update`, adding items under `client`
/// when `client` is given.
fn body_update(update: Vec<u8>, client: Option<u64>) -> Operation {
    let clients = client
        .map(|client| (Key::Unsigned(client), Cbor::Bytes(NONCE.to_vec())))
        .into_iter()
        .collect();
    let arguments = Cbor::Map(
        [
            (Key::Unsigned(1), Cbor::Bytes(update)),
            (Key::Unsigned(2), Cbor::Map(clients)),
        ]
        .into(),
    );
    Operation::new(
        TASK,
        "tasks".to_owned(),
        Some("body".to_owned()),
        UPDATE,
        Some(arguments),
    )
}

fn doc(client: u64) -> Doc {
    Doc::with_options(Options {
        client_id: ClientID::new(client),
        ..Options::default()
    })
}

fn apply(doc: &Doc, update: &[u8]) {
    let update = Update::decode_v1(update).unwrap();
    doc.transact_mut().apply_update(update).unwrap();
}

/// The update of what `doc` gained since `before`.
fn since(doc: &Doc, before: &StateVector) -> Vec<u8> {
    doc.transact().encode_state_as_update_v1(before)
}

#[test]
fn a_moved_rich_text_field_that_held_a_lost_operation_is_listed_among_the_conflicts() {
    let lock = lock();
    let model_id = ModelLock::from_json(&lock).expect("the lock").model.id.0;
    let db = now(Db::open_as(
        OpenOptions::new(&lock).in_memory(),
        [ME; 32],
        [ME ^ 0x80; 32],
        space::account(),
    ))
    .expect("the lock opens");

    // The admin shares a space with the member, with a task whose body
    // holds one paragraph.
    let mut shared = space::Space::with_model(ADMIN, [7; 16], model_id);
    let genesis = shared.genesis.clone();
    let add = shared.add(ADMIN, ME, CoreRole::Writer, &[&genesis]);
    let own = shared.envelope(ADMIN, ADMIN, &[&add]);
    let mine = shared.envelope(ADMIN, ME, &[&own]);
    let admin_client = client_id(&space::key(ADMIN).id(), &NONCE);
    let admin = doc(admin_client);
    let root = admin.get_or_insert_xml_fragment("default");
    let empty = StateVector::default();
    {
        let mut txn = admin.transact_mut();
        let paragraph = root.insert(&mut txn, 0, XmlElementPrelim::empty("paragraph"));
        paragraph.insert(&mut txn, 0, XmlTextPrelim::new("Base."));
    }
    let base = since(&admin, &empty);
    let task = shared.sealed(
        ADMIN,
        vec![
            field_op::create(TASK, "tasks"),
            body_update(base.clone(), Some(admin_client)),
        ],
    );
    receive(&db, &[&genesis, &add, &own, &mine, &task]);
    let space_id = SpaceId(shared.space);

    // The member types into the paragraph.
    let opened =
        rich_text::open(&mut db.lock(), space_id, "tasks", ObjectId(TASK), "body").unwrap();
    let lease = opened.binding.lease;
    let typing = doc(lease.client);
    apply(&typing, &opened.document);
    let before = typing.transact().state_vector();
    let typed_root = typing.get_or_insert_xml_fragment("default");
    {
        let mut txn = typing.transact_mut();
        let paragraph = typed_root.get(&txn, 0).unwrap().into_xml_element().unwrap();
        let text = paragraph.get(&txn, 0).unwrap().into_xml_text().unwrap();
        text.insert(&mut txn, 5, " Mine.");
    }
    let typed = RichTextUpdate {
        update: since(&typing, &before),
        client: lease.client,
        nonce: lease.nonce,
    };
    let handle = now(db.spaces().get(space_id)).unwrap();
    now(handle.update("tasks", ObjectId(TASK), [("body", typed.value())])).unwrap();

    // The admin, who has not seen the typing, deletes the paragraph.
    let before = admin.transact().state_vector();
    root.remove_range(&mut admin.transact_mut(), 0, 1);
    let deleted = shared.sealed(ADMIN, vec![body_update(since(&admin, &before), None)]);
    receive(&db, &[&deleted]);

    let personal = now(db.spaces().create()).unwrap();
    let object = now(handle.get("tasks", ObjectId(TASK))).unwrap().unwrap();
    let moved = now(handle.move_objects(&[object], &personal)).unwrap();
    assert_eq!(moved.conflicts, vec![(ObjectId(TASK), "body".to_owned())]);
}
