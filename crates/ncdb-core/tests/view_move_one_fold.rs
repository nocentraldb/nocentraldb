//! An object that becomes a trace, or stops being one, within the fold of
//! one change (NOC-542; RFC 0016): the fold remembers whether each object
//! is a trace instead of reading its state again for every operation, so
//! every way the `objects` table changes inside one fold must reach what it
//! remembers.
//! A move or a take-back in the same change as the object's fields, and a
//! revocation that un-folds a move and folds the object's fields again,
//! each end where a view that never remembered anything ends: the copies'
//! tables are equal byte for byte. The revived and displaced fields of a
//! take-back are in view_move.rs and rich_text_move.rs.

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::{Space, key};
use view_support::*;

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

const T: ObjectId = ObjectId([0x11; 16]);

/// The space the object moves to.
const NEW: SpaceId = SpaceId([0x5a; 16]);

/// An id the encodings ignore: a change's operations take theirs from it.
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

fn write(value: Value, replaces: BTreeSet<OperationId>) -> Write {
    Write {
        id: ANY,
        author: MemberId([0; 32]),
        stamp: Hlc(0),
        value,
        replaces,
    }
}

fn create() -> Operation {
    field_op::create(T, TASK)
}

fn title(value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    let value = Value::Text(value.to_owned());
    field_op::encode(&text, &write(value, BTreeSet::new()), T, TASK, TITLE)
}

fn move_to_new() -> Operation {
    let value = Value::Bytes(NEW.0.to_vec());
    field_op::move_object(T, TASK, &write(value, BTreeSet::new()))
}

/// Takes back the move `moved`.
fn take_back(moved: OperationId) -> Operation {
    field_op::move_object(T, TASK, &write(Value::Null, BTreeSet::from([moved])))
}

/// The id of the operation at `position` of `change`.
fn at(change: &Change, position: u64) -> OperationId {
    OperationId {
        change: change.id(),
        position,
    }
}

/// Alice founds the space with Bob as a writer and Carol as a reader, and
/// wraps the space key to Carol.
fn founded() -> (Space, Vec<Change>) {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = space.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let to_carol = space.envelope(ALICE, CAROL, &[&carol]);
    (space, vec![genesis, bob, carol, to_carol])
}

/// Carol's view after these changes, in this order, in `db`.
fn carol(space: &Space, db: &MemDb, changes: &[&Change]) -> (View, SealChecks) {
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = SealChecks::new(space.space, key(CAROL));
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (view, checks)
}

/// How many merge states and non-null titles the view keeps of T.
fn plaintext(db: &MemDb) -> (i64, i64) {
    let conn = db.connect();
    let count = |sql: &str| conn.query_row(sql, [T.0], |r| r.get(0)).unwrap();
    (
        count("SELECT count(*) FROM fs_task WHERE object_id = ?1"),
        count("SELECT count(*) FROM c_task WHERE object_id = ?1 AND f_title IS NOT NULL"),
    )
}

/// How many tasks the title index finds under `title`.
fn titled(view: &View, title: &str) -> usize {
    let key = [Value::Text(title.to_owned())];
    view.page(TASK, BY_TITLE, &key, None, 50)
        .unwrap()
        .rows
        .len()
}

#[test]
fn a_move_after_the_fields_in_one_change_leaves_a_trace() {
    let (mut space, mut changes) = founded();
    changes.push(space.sealed(ALICE, vec![create(), title("a"), move_to_new()]));
    let db = MemDb::new();
    let (view, _) = carol(&space, &db, &changes.iter().collect::<Vec<_>>());
    let row = view.get(space.space, TASK, T).unwrap().expect("the trace");
    assert_eq!(row.object.moved_to(), Some(NEW));
    assert!(row.fields.is_empty());
    assert_eq!(plaintext(&db), (0, 0));
}

#[test]
fn a_field_after_the_move_in_one_change_reaches_no_plaintext() {
    let (mut space, mut changes) = founded();
    changes.push(space.sealed(ALICE, vec![create(), move_to_new(), title("a")]));
    let db = MemDb::new();
    let (view, _) = carol(&space, &db, &changes.iter().collect::<Vec<_>>());
    let row = view.get(space.space, TASK, T).unwrap().expect("the trace");
    assert_eq!(row.object.moved_to(), Some(NEW));
    assert!(row.fields.is_empty());
    assert_eq!(plaintext(&db), (0, 0));
}

#[test]
fn a_take_back_and_a_field_in_one_change_fold_the_field() {
    let (mut space, mut changes) = founded();
    changes.push(space.sealed(ALICE, vec![create(), title("a")]));
    let moved = space.sealed(ALICE, vec![move_to_new()]);
    let back = space.sealed(ALICE, vec![take_back(at(&moved, 0)), title("b")]);
    changes.extend([moved, back]);
    let db = MemDb::new();
    let (view, _) = carol(&space, &db, &changes.iter().collect::<Vec<_>>());
    let row = view.get(space.space, TASK, T).unwrap().expect("the object");
    assert_eq!(row.object.moved_to(), None);
    assert!(row.fields.contains_key(TITLE));
    assert_eq!(titled(&view, "b"), 1, "the title is in its index");
    assert_eq!(plaintext(&db), (1, 1));
}

#[test]
fn a_revocation_that_un_folds_a_move_folds_the_fields_again() {
    let (mut space, mut changes) = founded();
    let created = space.sealed(ALICE, vec![create(), title("a")]);
    space.skip(BOB, 100);
    let moved = space.sealed(BOB, vec![move_to_new()]);
    let removal = space.remove(ALICE, BOB, 0, &[changes.last().unwrap()]);
    changes.push(created);

    // Carol folds Bob's move, then the removal un-folds it: its fold clears
    // T's state and folds Alice's create and title again.
    let late_db = MemDb::new();
    let late: Vec<&Change> = changes.iter().chain([&moved, &removal]).collect();
    let (late_view, _) = carol(&space, &late_db, &late);
    // A copy that never folded the move.
    let never_db = MemDb::new();
    let never: Vec<&Change> = changes.iter().chain([&removal]).collect();
    // An in-memory database lives as long as a connection to it.
    let _never_view = carol(&space, &never_db, &never);

    let row = late_view
        .get(space.space, TASK, T)
        .unwrap()
        .expect("the object");
    assert_eq!(row.object.moved_to(), None);
    assert!(row.fields.contains_key(TITLE));
    assert_eq!(titled(&late_view, "a"), 1, "the title is in its index");
    assert!(
        view_bytes(&late_db.connect()) == view_bytes(&never_db.connect()),
        "the tables equal those of a copy that never folded the move"
    );
}
