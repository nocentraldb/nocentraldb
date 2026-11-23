//! The trace of a moved object in the view (RFC 0016, "What the members of
//! the old space keep"; NOC-475): a moved object leaves every declared
//! index, the view keeps no plaintext of its fields, neither read values
//! nor merge states, as for an erased field, and a read by id gives the
//! trace, with the space it moved to and no fields. Taking the move back
//! folds its fields again from the log, those written while it was moved
//! included. Copies of the view converge byte for byte in any order of
//! arrival.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::State;
use ncdb_core::view::View;

use member::*;
use view_support::*;

/// The space the object moves to.
const NEW: SpaceId = SpaceId([0x5a; 16]);

fn write(value: Value, replaces: BTreeSet<OperationId>) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces,
    }
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

fn title(n: u8, title: &str) -> Operation {
    let write = write(Value::Text(title.into()), BTreeSet::new());
    field_op::encode(
        &BasicValue { kind: Kind::Text },
        &write,
        object(n),
        TASK,
        TITLE,
    )
}

fn rank(n: u8, rank: u64) -> Operation {
    let write = write(Value::Unsigned(rank), BTreeSet::new());
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    field_op::encode(&integer, &write, object(n), TASK, RANK)
}

/// Task `n` with a title and a rank.
fn task(n: u8, name: &str, r: u64) -> Vec<Operation> {
    vec![
        field_op::create(object(n), TASK),
        title(n, name),
        rank(n, r),
    ]
}

/// Moves task `n` to the new space.
fn move_to_new(n: u8) -> Operation {
    let write = write(Value::Bytes(NEW.0.to_vec()), BTreeSet::new());
    field_op::move_object(object(n), TASK, &write)
}

/// Takes back the move of task `n`, the operation `moved`.
fn take_back(n: u8, moved: OperationId) -> Operation {
    let write = write(Value::Null, BTreeSet::from([moved]));
    field_op::move_object(object(n), TASK, &write)
}

fn delete(n: u8) -> Operation {
    field_op::delete(object(n), TASK, &write(Value::Bool(true), BTreeSet::new()))
}

/// A view and the member that writes its changes, one at a time.
struct Copy {
    db: MemDb,
    view: View,
    control: Control,
    sequence: u64,
}

impl Copy {
    fn new() -> Copy {
        let db = MemDb::new();
        let mut view = View::open(&db.0, &lock()).unwrap();
        let mut control = Control::default();
        view.receive(&envelope(FOUNDER, 1, 0), &mut control)
            .unwrap();
        Copy {
            db,
            view,
            control,
            sequence: 1,
        }
    }

    /// Receives one change with these operations; returns the id of its
    /// first operation.
    fn receive(&mut self, operations: Vec<Operation>) -> OperationId {
        self.sequence += 1;
        let clock = Hlc(self.sequence);
        let payload = Payload::new(clock, BTreeSet::new(), operations).encode();
        let bytes = frame(sealed_header(FOUNDER, self.sequence, 0, payload), GOOD);
        self.view.receive(&bytes, &mut self.control).unwrap();
        OperationId {
            change: Change::decode(&bytes).unwrap().id(),
            position: 0,
        }
    }

    /// The first bytes of the object ids of every row of a page of 50.
    fn page(&self, index: &str) -> Vec<u8> {
        let page = self.view.page(TASK, index, &[], None, 50).unwrap();
        page.rows.iter().map(|(_, object)| object.0[0]).collect()
    }

    fn row(&self, n: u8) -> ncdb_core::view::Row {
        let row = self.view.get(SPACE, TASK, object(n)).unwrap();
        row.expect("a read by id finds the object")
    }

    /// How many merge states and non-null read values the view keeps of
    /// task `n`.
    fn plaintext(&self, n: u8) -> (i64, i64) {
        let conn = self.db.connect();
        let states = conn
            .query_row(
                "SELECT count(*) FROM fs_task WHERE space_id = ?1 AND object_id = ?2",
                (SPACE.0, object(n).0),
                |r| r.get(0),
            )
            .unwrap();
        let values = conn
            .query_row(
                "SELECT count(*) FROM c_task WHERE space_id = ?1 AND object_id = ?2
                 AND (f_title IS NOT NULL OR f_rank IS NOT NULL)",
                (SPACE.0, object(n).0),
                |r| r.get(0),
            )
            .unwrap();
        (states, values)
    }
}

#[test]
fn a_moved_object_leaves_every_index_and_a_read_by_id_gives_the_trace() {
    let mut copy = Copy::new();
    copy.receive([task(1, "a", 1), task(2, "b", 2)].concat());
    copy.receive(vec![move_to_new(2)]);
    assert_eq!(copy.page(BY_RANK), [1]);
    assert_eq!(copy.page(BY_NAME), [1]);
    let trace = copy.row(2);
    assert_eq!(trace.object.moved_to(), Some(NEW));
    assert!(trace.fields.is_empty(), "the trace shows no fields");
}

#[test]
fn the_view_keeps_no_plaintext_of_a_moved_objects_fields() {
    let mut copy = Copy::new();
    copy.receive(task(1, "a", 1));
    assert_eq!(copy.plaintext(1), (2, 1));
    copy.receive(vec![move_to_new(1)]);
    assert_eq!(copy.plaintext(1), (0, 0));
}

#[test]
fn a_field_written_after_the_move_reaches_no_plaintext() {
    let mut copy = Copy::new();
    copy.receive(task(1, "a", 1));
    copy.receive(vec![move_to_new(1)]);
    copy.receive(vec![title(1, "later")]);
    assert_eq!(copy.plaintext(1), (0, 0));
    assert!(copy.row(1).fields.is_empty());
}

#[test]
fn a_move_that_arrives_before_the_create_keeps_the_object_a_trace() {
    let mut copy = Copy::new();
    copy.receive(vec![move_to_new(2)]);
    copy.receive([task(1, "a", 1), task(2, "b", 2)].concat());
    assert_eq!(copy.page(BY_RANK), [1]);
    assert_eq!(copy.plaintext(2), (0, 0));
    assert_eq!(copy.row(2).object.moved_to(), Some(NEW));
}

#[test]
fn taking_the_move_back_folds_the_fields_again_from_the_log() {
    let mut copy = Copy::new();
    copy.receive(task(1, "a", 1));
    let moved = copy.receive(vec![move_to_new(1)]);
    copy.receive(vec![title(1, "later")]);
    copy.receive(vec![take_back(1, moved)]);
    assert_eq!(copy.page(BY_RANK), [1]);
    let row = copy.row(1);
    assert_eq!(row.object.moved_to(), None);
    assert_eq!(row.fields.len(), 2);
    assert_eq!(copy.plaintext(1), (2, 1));
    let later = [Value::Text("later".into())];
    let page = copy.view.page(TASK, BY_TITLE, &later, None, 50).unwrap();
    assert_eq!(
        page.rows.len(),
        1,
        "the title written while moved is folded"
    );
}

#[test]
fn a_deleted_object_whose_move_is_taken_back_stays_out_of_the_indexes() {
    let mut copy = Copy::new();
    copy.receive(task(1, "a", 1));
    let moved = copy.receive(vec![move_to_new(1)]);
    copy.receive(vec![delete(1)]);
    copy.receive(vec![take_back(1, moved)]);
    assert_eq!(copy.page(BY_RANK), Vec::<u8>::new());
    assert!(copy.row(1).object.is_deleted());
}

mod convergence {
    use super::*;
    use proptest::prelude::*;
    use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

    /// One copy of the view, with the frames it received.
    struct ViewCopy(Copy, Vec<Vec<u8>>);

    impl Default for ViewCopy {
        fn default() -> ViewCopy {
            ViewCopy(Copy::new(), Vec::new())
        }
    }

    /// A clone is a new view that receives the same frames.
    impl Clone for ViewCopy {
        fn clone(&self) -> ViewCopy {
            let mut copy = ViewCopy::default();
            for frame in &self.1 {
                copy.receive(frame);
            }
            copy
        }
    }

    impl ViewCopy {
        fn receive(&mut self, frame: &[u8]) {
            let copy = &mut self.0;
            copy.view.receive(frame, &mut copy.control).unwrap();
            self.1.push(frame.to_vec());
        }
    }

    impl std::fmt::Debug for ViewCopy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "ViewCopy({} frames)", self.1.len())
        }
    }

    impl Mergeable for ViewCopy {
        type Change = Vec<u8>;
        type Read = Vec<Option<ncdb_core::view::Row>>;

        fn apply(&mut self, op: &Op<Vec<u8>>) {
            self.receive(&op.body);
        }

        fn state_bytes(&self) -> Vec<u8> {
            view_bytes(&self.0.db.connect())
        }

        fn read(&self) -> Self::Read {
            [1, 2]
                .into_iter()
                .map(|n| self.0.view.get(SPACE, TASK, object(n)).unwrap())
                .collect()
        }

        fn outcome(&self, _: OpId) -> Outcome {
            Outcome::Visible
        }
    }

    fn operation() -> BoxedStrategy<Operation> {
        let n = prop_oneof![Just(1u8), Just(2u8)];
        let back = |n: u8| {
            let write = write(Value::Null, BTreeSet::new());
            field_op::move_object(object(n), TASK, &write)
        };
        prop_oneof![
            n.clone().prop_map(|n| field_op::create(object(n), TASK)),
            n.clone().prop_map(move_to_new),
            n.clone().prop_map(back),
            n.clone().prop_map(delete),
            (n.clone(), prop_oneof![Just("a"), Just("b")]).prop_map(|(n, t)| title(n, t)),
            (n, 0..3u64).prop_map(|(n, r)| rank(n, r)),
        ]
        .boxed()
    }

    fn changes(author: u8) -> BoxedStrategy<Vec<u8>> {
        (1..6u64, 1..4u64, prop::collection::vec(operation(), 1..4))
            .prop_map(move |(sequence, clock, operations)| {
                let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
                frame(sealed_header(author, sequence, 0, payload), GOOD)
            })
            .boxed()
    }

    #[test]
    fn copies_of_the_view_converge_with_moves_in_any_order() {
        let config = Config {
            members: 3,
            max_changes_per_member: 3,
            cases: 64,
            ..Config::default()
        };
        let result = convergence::check::<ViewCopy, _>(&config, &independent(changes));
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn the_new_space_reads_where_the_object_came_from() {
    // RFC 0016, "Formats": `$movedFrom` is hidden, so no model declares it,
    // and the view keeps it with the object, not as a field.
    let mut copy = Copy::new();
    let old = Value::Map(
        [(
            ncdb_core::cbor::Key::Unsigned(1),
            Value::Bytes(vec![0x0d; 16]),
        )]
        .into(),
    );
    let mut operations = task(1, "a", 1);
    operations.push(field_op::moved_from(
        object(1),
        TASK,
        &write(old, BTreeSet::new()),
    ));
    let first = copy.receive(operations);
    let row = copy.row(1);
    assert_eq!(row.object.moved_from(), Some(SpaceId([0x0d; 16])));
    assert_eq!(row.fields.len(), 2);
    assert_eq!(
        copy.view.state(SPACE, first.change),
        Some(State::Folded { parked: vec![] }),
        "no operation parks"
    );
}
