//! Later edits (RFC 0016, "Later edits"): an operation on a moved object,
//! in the old space, that is not in the causal past of the move that won.
//! Every member of the old space computes them from the old space's
//! changes alone, through the changes' dependencies, so every member finds
//! the same set in any order of arrival. A losing concurrent move is a
//! conflict of the move, not a later edit; an edit in the move's own change
//! or its past is none either.

mod support;

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::moves;
use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

use member::*;

const T: ObjectId = ObjectId([0x11; 16]);
const NEW: SpaceId = SpaceId([0x5a; 16]);
const OTHER_NEW: SpaceId = SpaceId([0x5b; 16]);

fn write(value: Value) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn title(value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    field_op::encode(
        &text,
        &write(Value::Text(value.into())),
        T,
        COLLECTION,
        FIELD,
    )
}

fn move_to(space: SpaceId) -> Operation {
    field_op::move_object(T, COLLECTION, &write(Value::Bytes(space.0.to_vec())))
}

fn delete() -> Operation {
    field_op::delete(T, COLLECTION, &write(Value::Bool(true)))
}

/// A change of `author` at `clock` that depends on `after`.
fn change(
    author: u8,
    sequence: u64,
    clock: u64,
    after: &[&[u8]],
    operations: Vec<Operation>,
) -> Vec<u8> {
    let dependencies = after
        .iter()
        .map(|f| Change::decode(f).unwrap().id())
        .collect();
    let payload = Payload::new(Hlc(clock), dependencies, operations).encode();
    frame(sealed_header(author, sequence, 0, payload), GOOD)
}

fn id(frame: &[u8], position: u64) -> OperationId {
    OperationId {
        change: Change::decode(frame).unwrap().id(),
        position,
    }
}

/// A member of the space that receives `frames`, in this order.
fn copy(frames: &[&[u8]]) -> Copy {
    let mut copy = Copy::default();
    for frame in frames {
        copy.receive(frame);
    }
    copy
}

/// The ids of the later edits of `T`.
fn later(copy: &Copy) -> Vec<OperationId> {
    let edits = moves::later_edits(&copy.0.log, &copy.0.control);
    edits
        .get(&T)
        .map(|e| e.iter().map(|edit| edit.id).collect())
        .unwrap_or_default()
}

#[test]
fn an_edit_the_mover_had_seen_is_no_later_edit() {
    let created = change(
        1,
        1,
        1,
        &[],
        vec![field_op::create(T, COLLECTION), title("a")],
    );
    let edited = change(2, 1, 2, &[&created], vec![title("b")]);
    let moved = change(1, 2, 3, &[&edited], vec![move_to(NEW)]);
    assert_eq!(later(&copy(&[&created, &edited, &moved])), vec![]);
}

#[test]
fn an_edit_concurrent_with_the_move_is_a_later_edit() {
    let created = change(
        1,
        1,
        1,
        &[],
        vec![field_op::create(T, COLLECTION), title("a")],
    );
    let moved = change(1, 2, 3, &[&created], vec![move_to(NEW)]);
    let offline = change(2, 1, 2, &[&created], vec![title("offline")]);
    let edit = moves::later_edits(&copy(&[&created, &moved, &offline]).0.log, &Control::all());
    let edits = &edit[&T];
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].id, id(&offline, 0));
    assert_eq!(edits[0].operation, title("offline"));
}

#[test]
fn an_edit_written_after_the_move_is_a_later_edit_and_so_is_a_delete() {
    // An old core shows the object as live and writes on it, after it
    // received the move (RFC 0016, "Formats").
    let created = change(
        1,
        1,
        1,
        &[],
        vec![field_op::create(T, COLLECTION), title("a")],
    );
    let moved = change(1, 2, 2, &[&created], vec![move_to(NEW)]);
    let after = change(2, 1, 3, &[&moved], vec![title("old core"), delete()]);
    assert_eq!(
        later(&copy(&[&created, &moved, &after])),
        vec![id(&after, 0), id(&after, 1)]
    );
}

#[test]
fn an_edit_in_the_moves_own_change_is_no_later_edit() {
    let created = change(1, 1, 1, &[], vec![field_op::create(T, COLLECTION)]);
    let moved = change(1, 2, 2, &[&created], vec![title("last"), move_to(NEW)]);
    assert_eq!(later(&copy(&[&created, &moved])), vec![]);
}

#[test]
fn a_losing_move_is_no_later_edit_and_the_winner_decides_the_past() {
    let created = change(1, 1, 1, &[], vec![field_op::create(T, COLLECTION)]);
    let edited = change(3, 1, 2, &[&created], vec![title("seen by the winner")]);
    let loser = change(1, 2, 3, &[&created], vec![move_to(OTHER_NEW)]);
    let winner = change(2, 1, 4, &[&edited], vec![move_to(NEW)]);
    assert_eq!(later(&copy(&[&created, &edited, &loser, &winner])), vec![]);
}

#[test]
fn a_move_taken_back_leaves_no_later_edits() {
    let created = change(1, 1, 1, &[], vec![field_op::create(T, COLLECTION)]);
    let moved = change(1, 2, 2, &[&created], vec![move_to(NEW)]);
    let offline = change(2, 1, 3, &[&created], vec![title("offline")]);
    let back = field_op::move_object(T, COLLECTION, &write(Value::Null));
    let taken_back = change(1, 3, 4, &[&moved], vec![back]);
    assert!(
        moves::later_edits(
            &copy(&[&created, &moved, &offline, &taken_back]).0.log,
            &Control::all()
        )
        .is_empty()
    );
}

/// One member's copy, with the frames it received.
#[derive(Clone, Debug)]
struct Copy(Member, Vec<Vec<u8>>);

impl Default for Copy {
    fn default() -> Copy {
        let mut m = Member::default();
        m.control = Control::all();
        Copy(m, Vec::new())
    }
}

impl Copy {
    fn receive(&mut self, frame: &[u8]) {
        let m = &mut self.0;
        m.log
            .receive(frame, &mut m.control, &mut m.fold)
            .expect("every frame is appended");
        self.1.push(frame.to_vec());
    }
}

impl Control {
    /// A control state that lets authors 0 to 3 write at generation 0.
    fn all() -> Control {
        let mut control = Control::default();
        control.keys.insert(0);
        control.members = (0..4).map(|n| (member(n), 0)).collect();
        control
    }
}

impl Mergeable for Copy {
    type Change = Vec<u8>;
    /// The later edits of each moved object.
    type Read = BTreeMap<ObjectId, Vec<OperationId>>;

    fn apply(&mut self, op: &Op<Vec<u8>>) {
        self.receive(&op.body);
    }

    fn state_bytes(&self) -> Vec<u8> {
        format!("{:?}", (&self.0.fold.objects, self.read())).into_bytes()
    }

    fn read(&self) -> Self::Read {
        moves::later_edits(&self.0.log, &self.0.control)
            .into_iter()
            .map(|(object, edits)| (object, edits.into_iter().map(|e| e.id).collect()))
            .collect()
    }

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// What a member does: create the object, write its title, move it to one
/// of two spaces, take the move back, or delete it.
#[derive(Clone, Debug)]
enum Raw {
    Create,
    Title(u8),
    Move(bool),
    TakeBack,
    Delete,
}

/// Each change depends on every change its author had seen, so the causal
/// past of a move is what its author had seen.
struct Causal;

impl Generator<Copy> for Causal {
    type Raw = (u64, Raw);

    fn raw(&self, _: u8) -> BoxedStrategy<(u64, Raw)> {
        let raw = prop_oneof![
            Just(Raw::Create),
            (0..3u8).prop_map(Raw::Title),
            any::<bool>().prop_map(Raw::Move),
            Just(Raw::TakeBack),
            Just(Raw::Delete),
        ];
        (1..5u64, raw).boxed()
    }

    fn make(&self, author: &Author<'_, Copy>, (clock, raw): &(u64, Raw)) -> Vec<u8> {
        let operation = match raw {
            Raw::Create => field_op::create(T, COLLECTION),
            Raw::Title(n) => title(&n.to_string()),
            Raw::Move(first) => move_to(if *first { NEW } else { OTHER_NEW }),
            Raw::TakeBack => field_op::move_object(T, COLLECTION, &write(Value::Null)),
            Raw::Delete => delete(),
        };
        let seen: Vec<&[u8]> = author.seen.iter().map(|op| op.body.as_slice()).collect();
        let sequence = 1 + seen
            .iter()
            .filter(|f| Change::decode(f).unwrap().header().author == member(author.member))
            .count() as u64;
        change(author.member, sequence, *clock, &seen, vec![operation])
    }
}

#[test]
fn later_edits_are_the_same_on_every_copy_in_any_order_of_arrival() {
    let config = Config {
        members: 3,
        max_changes_per_member: 4,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(&config, &Causal);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}
