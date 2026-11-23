//! Where an object moved (RFC 0016, "What a move is", "Two moves at once"
//! and "Formats"): the move is a basic value of the object, so a copy reads
//! the winning destination, keeps a losing concurrent move as a readable
//! conflict, takes a move back with `null`, and keeps the move apart from
//! the deletion. The convergence harness (NOC-9) delivers moves, take-backs
//! and deletes to every copy in different orders and with duplicates.

mod support;

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{self, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::object::{Object, ObjectOperation};
use proptest::prelude::*;
use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

use member::*;

/// The object every change moves.
const T: ObjectId = ObjectId([0x11; 16]);
/// Two spaces the object can move to.
const U: SpaceId = SpaceId([0x55; 16]);
const V: SpaceId = SpaceId([0x56; 16]);

fn op_id(position: u64) -> OperationId {
    OperationId {
        change: ChangeId([position as u8; 32]),
        position,
    }
}

fn write(position: u64, stamp: u64, value: Value, replaces: &[u64]) -> Write {
    Write {
        id: op_id(position),
        author: member(1),
        stamp: Hlc(stamp),
        value,
        replaces: replaces.iter().map(|p| op_id(*p)).collect(),
    }
}

fn to(space: SpaceId) -> Value {
    Value::Bytes(space.0.to_vec())
}

#[test]
fn an_object_never_moved_reads_no_destination() {
    let object = Object::default();
    assert_eq!(object.moved_to(), None);
    assert_eq!(object.moving().value, None);
}

#[test]
fn a_move_names_the_new_space() {
    let mut object = Object::default();
    object.apply(&ObjectOperation::Move(write(1, 1, to(U), &[])));
    assert_eq!(object.moved_to(), Some(U));
    assert!(!object.is_deleted());
}

#[test]
fn a_move_taken_back_reads_no_destination() {
    let mut object = Object::default();
    object.apply(&ObjectOperation::Move(write(1, 1, to(U), &[])));
    object.apply(&ObjectOperation::Move(write(2, 2, Value::Null, &[1])));
    assert_eq!(object.moved_to(), None);
}

#[test]
fn two_moves_at_once_keep_one_winner_and_the_other_as_a_conflict() {
    let mut object = Object::default();
    object.apply(&ObjectOperation::Move(write(1, 1, to(U), &[])));
    object.apply(&ObjectOperation::Move(write(2, 2, to(V), &[])));
    assert_eq!(object.moved_to(), Some(V));
    let read = object.moving();
    assert_eq!(read.conflicts.len(), 1);
    assert_eq!(read.conflicts[0].value, to(U));
}

#[test]
fn a_move_and_a_delete_are_independent_values() {
    let mut object = Object::default();
    object.apply(&ObjectOperation::Move(write(1, 1, to(U), &[])));
    object.apply(&ObjectOperation::Delete(write(2, 2, Value::Bool(true), &[])));
    assert_eq!(object.moved_to(), Some(U));
    assert!(object.is_deleted());
}

#[test]
fn a_destination_that_is_no_space_id_reads_as_not_moved() {
    let mut object = Object::default();
    object.apply(&ObjectOperation::Move(write(1, 1, Value::Text("x".into()), &[])));
    assert_eq!(object.moved_to(), None);
}

/// One copy: a member whose control state lets every author write.
#[derive(Clone, Debug)]
struct Copy(Member);

impl Default for Copy {
    fn default() -> Copy {
        let mut m = Member::default();
        m.control.keys.insert(0);
        m.control.members = (1..4).map(|n| (member(n), 0)).collect();
        Copy(m)
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    /// Where `T` moved, the move with its conflicts, and whether it is
    /// deleted.
    type Read = (Option<SpaceId>, basic::Read, bool);

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        let m = &mut self.0;
        m.log
            .receive(&op.body, &mut m.control, &mut m.fold)
            .expect("every generated frame is appended");
    }

    fn state_bytes(&self) -> Vec<u8> {
        format!("{:?}", self.0.fold.objects).into_bytes()
    }

    fn read(&self) -> Self::Read {
        let object = self.0.fold.objects.get(&T).cloned().unwrap_or_default();
        (object.moved_to(), object.moving(), object.is_deleted())
    }

    /// The copy keeps every move: a losing move stays a conflict.
    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// A move of `T` to `value`, or a delete of it, replacing nothing.
fn object_write(value: Value, delete: bool) -> Operation {
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    };
    if delete {
        field_op::delete(T, COLLECTION, &write)
    } else {
        field_op::move_object(T, COLLECTION, &write)
    }
}

/// A change `author` makes: a create of `T`, a move of it to one of two
/// spaces, a take-back, a delete or restore, or a move whose destination
/// is no space id. Clocks repeat, so the tie-break by author and position
/// is exercised.
fn change(author: u8) -> BoxedStrategy<Vec<u8>> {
    let operation = prop_oneof![
        Just(field_op::create(T, COLLECTION)),
        Just(object_write(to(U), false)),
        Just(object_write(to(V), false)),
        Just(object_write(Value::Null, false)),
        Just(object_write(Value::Text("x".into()), false)),
        any::<bool>().prop_map(|d| object_write(Value::Bool(d), true)),
    ];
    (1..6u64, 1..4u64, prop::collection::vec(operation, 1..3))
        .prop_map(move |(sequence, clock, operations)| {
            let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
            frame(sealed_header(author, sequence, 0, payload), GOOD)
        })
        .boxed()
}

#[test]
fn moves_in_any_order_end_in_the_same_destination_on_every_copy() {
    let config = Config {
        members: 4,
        max_changes_per_member: 4,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &independent(change));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
