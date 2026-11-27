//! How links and references resolve to a moved object (RFC 0016, "Links to
//! the moved object" and "References to the moved object").
//!
//! A link in the old space to the moved object finds the trace and reads
//! `moved`, with the new space, whether or not the trace was deleted too.
//! A reference from another space that names the old space follows the trace, up to 8 in a
//! row so that a cycle of moves ends, and resolves where the object is now:
//! ok, with the space where it found it, not accessible without a copy of
//! that space, or missing.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::{self, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::link::{Link, Status, Target};
use ncdb_core::object::{Create, Object};
use ncdb_core::reference::{self, Reference};

const NOTE: ObjectId = ObjectId([7; 16]);

/// The space of the object that holds each reference below: none of the
/// spaces the note lives in or moves to, so every reference is cross-space
/// (RFC 0018, "One function, two cases").
const HOLDER: SpaceId = SpaceId([99; 16]);

fn space(n: u8) -> SpaceId {
    SpaceId([n; 16])
}

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn write(n: u8, value: Value, replaces: &[u8]) -> Write {
    Write {
        id: op(n),
        author: MemberId([1; 32]),
        stamp: Hlc(u64::from(n)),
        value,
        replaces: replaces.iter().map(|r| op(*r)).collect(),
    }
}

fn notes() -> BTreeSet<String> {
    ["notes".to_owned()].into()
}

/// The note, created in `notes`.
fn note() -> Object {
    let mut object = Object::default();
    object.create(&Create {
        id: op(1),
        author: MemberId([1; 32]),
        stamp: Hlc(1),
        collection: "notes".into(),
    });
    object
}

/// The note, moved to `to`.
fn trace(to: SpaceId) -> Object {
    let mut object = note();
    object.set_moved(&write(2, Value::Bytes(to.0.to_vec()), &[]));
    object
}

/// A copy of one space that holds `object` as the note.
fn copy(object: Object) -> BTreeMap<ObjectId, Object> {
    [(NOTE, object)].into()
}

/// What a link to the note reads in `copy`.
fn link_status(copy: &BTreeMap<ObjectId, Object>) -> Status {
    let link = Link {
        collections: notes(),
    };
    let mut state = basic::State::default();
    let target = Target {
        space: space(1),
        object: NOTE,
    };
    let set = link
        .set(
            &state,
            op(9),
            MemberId([1; 32]),
            Hlc(9),
            space(1),
            Some(target),
        )
        .unwrap();
    link.apply(&mut state, &set);
    link.resolve(&state, copy).value.unwrap().status
}

#[test]
fn a_link_to_a_moved_object_reads_moved_with_the_new_space() {
    assert_eq!(link_status(&copy(trace(space(2)))), Status::Moved(space(2)));
}

#[test]
fn a_link_reads_ok_again_when_the_move_is_taken_back() {
    let mut object = trace(space(2));
    object.set_moved(&write(3, Value::Null, &[2]));
    assert_eq!(link_status(&copy(object)), Status::Ok);
}

#[test]
fn a_deleted_trace_still_reads_moved() {
    // A delete written after the move in the old space is a later edit;
    // the object lives on in the new space (RFC 0016, "Formats").
    let mut object = trace(space(2));
    object.set_deleted(&write(3, Value::Bool(true), &[]));
    assert_eq!(link_status(&copy(object)), Status::Moved(space(2)));
}

/// What a reference to the note in `space(1)` reads against `copies`.
fn reference_read(copies: &BTreeMap<SpaceId, BTreeMap<ObjectId, Object>>) -> reference::Resolution {
    let field = Reference {
        collections: notes(),
    };
    let mut state = basic::State::default();
    let target = Target {
        space: space(1),
        object: NOTE,
    };
    let set = field.set(&state, op(9), MemberId([1; 32]), Hlc(9), Some(target));
    field.apply(&mut state, &set);
    field.resolve(HOLDER, &state, copies).value.unwrap()
}

#[test]
fn a_reference_follows_the_trace_and_gives_the_space_it_found_the_object_in() {
    let copies = [(space(1), copy(trace(space(2)))), (space(2), copy(note()))].into();
    let read = reference_read(&copies);
    assert_eq!(read.status, reference::Status::Ok);
    assert_eq!(read.found, Some(space(2)));
}

#[test]
fn a_reference_that_finds_the_object_where_it_was_written_names_that_space() {
    let copies = [(space(1), copy(note()))].into();
    assert_eq!(reference_read(&copies).found, Some(space(1)));
}

#[test]
fn a_reference_follows_traces_one_after_the_other() {
    let copies = [
        (space(1), copy(trace(space(2)))),
        (space(2), copy(trace(space(3)))),
        (space(3), copy(note())),
    ]
    .into();
    let read = reference_read(&copies);
    assert_eq!(read.status, reference::Status::Ok);
    assert_eq!(read.found, Some(space(3)));
}

#[test]
fn a_reference_reads_not_accessible_without_a_copy_of_the_new_space() {
    let copies = [(space(1), copy(trace(space(2))))].into();
    let read = reference_read(&copies);
    assert_eq!(read.status, reference::Status::NotAccessible);
    assert_eq!(read.found, None);
}

#[test]
fn a_member_without_the_old_space_reads_not_accessible_even_with_the_new_one() {
    let copies = [(space(2), copy(note()))].into();
    assert_eq!(
        reference_read(&copies).status,
        reference::Status::NotAccessible
    );
}

#[test]
fn a_reference_reads_missing_when_the_new_space_lacks_the_object() {
    let copies = [
        (space(1), copy(trace(space(2)))),
        (space(2), BTreeMap::new()),
    ]
    .into();
    assert_eq!(reference_read(&copies).status, reference::Status::Missing);
}

#[test]
fn a_cycle_of_moves_ends_after_8_traces_and_reads_missing() {
    let copies = [
        (space(1), copy(trace(space(2)))),
        (space(2), copy(trace(space(1)))),
    ]
    .into();
    assert_eq!(reference_read(&copies).status, reference::Status::Missing);
}

#[test]
fn a_ninth_trace_in_a_row_is_not_followed() {
    // Spaces 1 to 9 each hold a trace to the next; space 10 holds the note.
    let mut copies: BTreeMap<_, _> = (1..=9)
        .map(|n| (space(n), copy(trace(space(n + 1)))))
        .collect();
    copies.insert(space(10), copy(note()));
    assert_eq!(reference_read(&copies).status, reference::Status::Missing);
    // With 8 traces in a row, the ninth space is reached.
    copies.remove(&space(1));
    let field = Reference {
        collections: notes(),
    };
    let mut state = basic::State::default();
    let target = Target {
        space: space(2),
        object: NOTE,
    };
    let set = field.set(&state, op(9), MemberId([1; 32]), Hlc(9), Some(target));
    field.apply(&mut state, &set);
    let read = field.resolve(HOLDER, &state, &copies).value.unwrap();
    assert_eq!(read.status, reference::Status::Ok);
    assert_eq!(read.found, Some(space(10)));
}
