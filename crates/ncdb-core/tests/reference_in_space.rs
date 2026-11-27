//! An in-space reference, one whose space id is the space of the object that
//! holds it, resolves as a link does (RFC 0018, "One function, two cases"):
//! from that space's copy alone, to ok, deleted, missing, wrong collection
//! or moved. Its status is a function of the space's changes, so every
//! member of the space reads the same, whichever other spaces it holds.
//! Cross-space references keep RFC 0004's three states (`reference.rs`).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::basic::{State, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::object::{Create, Object};
use ncdb_core::reference::{self, Reference, Status, Target};

/// The space of the object that holds the reference, and of its target.
const HOME: SpaceId = SpaceId([1; 16]);
/// Another space, where the target may move.
const ELSEWHERE: SpaceId = SpaceId([2; 16]);
const NOTE: ObjectId = ObjectId([7; 16]);

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn field() -> Reference {
    Reference {
        collections: ["notes".to_owned()].into(),
    }
}

type Copies = BTreeMap<SpaceId, BTreeMap<ObjectId, Object>>;

fn created(collection: &str) -> Object {
    let mut object = Object::default();
    object.create(&Create {
        id: op(1),
        author: MemberId([1; 32]),
        stamp: Hlc(5),
        collection: collection.to_owned(),
    });
    object
}

fn write(n: u8, value: Value) -> Write {
    Write {
        id: op(n),
        author: MemberId([9; 32]),
        stamp: Hlc(20),
        value,
        replaces: Default::default(),
    }
}

/// A reference field written once with the note in `space`.
fn to(space: SpaceId) -> State {
    let mut state = State::default();
    let set = field().set(
        &state,
        op(50),
        MemberId([1; 32]),
        Hlc(10),
        Some(Target {
            space,
            object: NOTE,
        }),
    );
    field().apply(&mut state, &set);
    state
}

/// What the reference held in `HOME` reads against `copies`.
fn read(state: &State, copies: &Copies) -> reference::Resolution {
    field()
        .resolve(HOME, state, copies)
        .value
        .expect("a live write")
}

fn home(note: Object) -> Copies {
    [(HOME, [(NOTE, note)].into())].into()
}

#[test]
fn a_target_in_its_own_space_reads_ok_and_names_that_space() {
    let read = read(&to(HOME), &home(created("notes")));
    assert_eq!((read.status, read.found), (Status::Ok, Some(HOME)));
}

#[test]
fn a_deleted_target_reads_deleted_not_missing() {
    let mut note = created("notes");
    note.set_deleted(&write(2, Value::Bool(true)));
    assert_eq!(read(&to(HOME), &home(note)).status, Status::Deleted);
}

#[test]
fn a_target_the_space_does_not_hold_reads_missing() {
    let copies: Copies = [(HOME, BTreeMap::new())].into();
    let read = read(&to(HOME), &copies);
    assert_eq!((read.status, read.found), (Status::Missing, None));
}

#[test]
fn a_target_in_a_collection_the_field_does_not_list_reads_wrong_collection() {
    assert_eq!(
        read(&to(HOME), &home(created("tasks"))).status,
        Status::WrongCollection
    );
}

#[test]
fn a_moved_target_reads_moved_with_the_new_space_and_is_not_followed() {
    let mut note = created("notes");
    note.set_moved(&write(3, Value::Bytes(ELSEWHERE.0.to_vec())));
    let mut copies = home(note);
    // The member holds the new space too, with the note in it: a link's
    // status does not follow the trace, and neither does an in-space
    // reference's.
    copies.insert(ELSEWHERE, [(NOTE, created("notes"))].into());
    let read = read(&to(HOME), &copies);
    assert_eq!(
        (read.status, read.found),
        (Status::Moved(ELSEWHERE), None)
    );
}

#[test]
fn every_member_of_the_space_reads_the_same_whatever_else_it_holds() {
    let mut note = created("notes");
    note.set_moved(&write(3, Value::Bytes(ELSEWHERE.0.to_vec())));
    let only_home = home(note);
    let mut with_elsewhere = only_home.clone();
    with_elsewhere.insert(ELSEWHERE, [(NOTE, created("notes"))].into());
    let state = to(HOME);
    assert_eq!(
        field().resolve(HOME, &state, &only_home),
        field().resolve(HOME, &state, &with_elsewhere)
    );
}

#[test]
fn the_same_value_held_in_another_space_is_cross_space_and_follows_the_trace() {
    // The reference names HOME, but its object lives in ELSEWHERE: a
    // reference's three states, following the trace (RFC 0004).
    let mut note = created("notes");
    note.set_moved(&write(3, Value::Bytes(ELSEWHERE.0.to_vec())));
    let mut copies = home(note);
    copies.insert(ELSEWHERE, [(NOTE, created("notes"))].into());
    let read = field()
        .resolve(ELSEWHERE, &to(HOME), &copies)
        .value
        .expect("a live write");
    assert_eq!((read.status, read.found), (Status::Ok, Some(ELSEWHERE)));
}

#[test]
fn a_deleted_target_in_another_space_still_reads_missing() {
    let mut note = created("notes");
    note.set_deleted(&write(2, Value::Bool(true)));
    let read = field()
        .resolve(ELSEWHERE, &to(HOME), &home(note))
        .value
        .expect("a live write");
    assert_eq!(read.status, Status::Missing);
}
