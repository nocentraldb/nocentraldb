//! The fold into the local view on SQLite (NOC-69; RFC 0006, "One
//! change"): creates and deletes go into the objects, field operations into
//! `fs_<collection>` with the read value in `c_<collection>`, with the same
//! results as the in-memory fold of the same changes.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

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
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::link::{self, Link};
use ncdb_core::log::Log;
use ncdb_core::set::{Action, Edit, Set};
use ncdb_core::view::View;
use rusqlite::types::Value as Sql;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);
const U: ObjectId = ObjectId([0x22; 16]);

/// An id the encodings ignore: a change's operations take theirs from it.
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

fn write(value: Value) -> Write {
    Write {
        id: ANY,
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn title(object: ObjectId, value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    let value = Value::Text(value.to_owned());
    field_op::encode(&text, &write(value), object, TASK, TITLE)
}

fn rank(object: ObjectId, value: Value) -> Operation {
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    field_op::encode(&integer, &write(value), object, TASK, RANK)
}

fn owner(object: ObjectId, target: ObjectId) -> Operation {
    let link = Link {
        collections: [PROJECT.to_owned()].into(),
    };
    field_op::encode(&link, &write(link::value(target)), object, TASK, OWNER)
}

fn tag(object: ObjectId, element: &str, add: bool) -> Operation {
    let action = if add {
        Action::Add {
            replaces: BTreeSet::new(),
        }
    } else {
        Action::Remove {
            observed: BTreeSet::new(),
        }
    };
    let edit = Edit {
        id: ANY,
        author: member(0),
        stamp: Hlc(0),
        element: Value::Text(element.to_owned()),
        action,
    };
    field_op::encode(&Set { kind: Kind::Text }, &edit, object, TASK, TAGS)
}

fn delete(object: ObjectId, deleted: bool) -> Operation {
    field_op::delete(object, TASK, &write(Value::Bool(deleted)))
}

/// A well-signed sealed change of `author` at `clock`, generation 0.
fn change(author: u8, sequence: u64, clock: u64, operations: Vec<Operation>) -> Vec<u8> {
    let payload = Payload::new(Hlc(clock), BTreeSet::new(), operations).encode();
    frame(sealed_header(author, sequence, 0, payload), GOOD)
}

/// One member's copy twice: in the SQLite view and in the in-memory fold.
struct Both {
    db: MemDb,
    view: View,
    view_control: Control,
    log: Log,
    control: Control,
    reference: Reference,
    frames: Vec<Vec<u8>>,
}

impl Both {
    /// A copy whose control state lets members 0 to 3 write at generation
    /// 0.
    fn new() -> Both {
        let mut control = Control::default();
        control.keys.insert(0);
        control.members = (1..4).map(|n| (member(n), 0)).collect();
        let db = MemDb::new();
        Both {
            view: View::open(&db.0, model()).unwrap(),
            db,
            view_control: control.clone(),
            log: Log::new(SPACE),
            control,
            reference: Reference::new(model()),
            frames: Vec::new(),
        }
    }

    fn receive(&mut self, frame: &[u8]) {
        self.view.receive(frame, &mut self.view_control).unwrap();
        self.log
            .receive(frame, &mut self.control, &mut self.reference)
            .unwrap();
        self.frames.push(frame.to_vec());
    }

    /// Checks that the view reads what the in-memory fold holds: the same
    /// state for every change, and the same object and fields for every
    /// object of every collection.
    fn assert_same(&self) {
        for frame in &self.frames {
            let id = Change::decode(frame).unwrap().id();
            assert_eq!(self.view.state(SPACE, id), self.log.state(id).cloned());
        }
        for collection in [TASK, PROJECT] {
            for object in [T, U] {
                assert_eq!(
                    self.view.get(SPACE, collection, object).unwrap(),
                    self.reference.row(collection, object),
                    "{collection} {object:?}"
                );
            }
        }
    }

    /// The value of a column of `c_task` for `object`.
    fn column(&self, column: &str, object: ObjectId) -> Sql {
        self.db
            .connect()
            .query_row(
                &format!("SELECT {column} FROM c_task WHERE space_id = ?1 AND object_id = ?2"),
                (SPACE.0, object.0),
                |r| r.get(0),
            )
            .unwrap()
    }
}

#[test]
fn a_create_and_field_writes_land_in_the_objects_and_the_tables_of_the_collection() {
    let mut both = Both::new();
    both.receive(&change(
        1,
        1,
        5,
        vec![
            field_op::create(T, TASK),
            title(T, "Ship v1"),
            rank(T, Value::Unsigned(7)),
            owner(T, U),
            tag(T, "urgent", true),
        ],
    ));
    both.assert_same();
    let row = both.view.get(SPACE, TASK, T).unwrap().unwrap();
    assert!(row.object.exists());
    assert_eq!(row.fields.len(), 4);
    // The read values are in the columns, as SQLite types.
    assert_eq!(both.column("f_title", T), Sql::Text("Ship v1".into()));
    assert_eq!(both.column("f_rank", T), Sql::Integer(7));
    assert_eq!(both.column("f_owner", T), Sql::Blob(U.0.to_vec()));
}

#[test]
fn a_later_write_replaces_the_read_value_in_the_column_and_a_clear_empties_it() {
    let mut both = Both::new();
    both.receive(&change(1, 1, 5, vec![title(T, "draft"), rank(T, Value::Negative(2))]));
    assert_eq!(both.column("f_rank", T), Sql::Integer(-3));
    both.receive(&change(2, 1, 6, vec![title(T, "final")]));
    assert_eq!(both.column("f_title", T), Sql::Text("final".into()));
    both.receive(&change(3, 1, 7, vec![title(T, "x"), rank(T, Value::Null)]));
    assert_eq!(both.column("f_rank", T), Sql::Null);
    // A value outside the field's schema stays in the state, readable as
    // unknown, and leaves the column empty.
    both.receive(&change(3, 2, 8, vec![rank(T, Value::Text("high".into()))]));
    assert_eq!(both.column("f_rank", T), Sql::Null);
    both.assert_same();
}

#[test]
fn deletes_operations_that_park_and_writes_before_the_create_fold_as_in_memory() {
    let mut both = Both::new();
    let malformed = Operation::new(T, TASK.into(), None, 0, Some(Value::Null));
    let unknown_field = Operation::new(T, TASK.into(), Some("nope".into()), 1, None);
    both.receive(&change(1, 1, 5, vec![title(U, "before"), delete(T, true)]));
    both.receive(&change(2, 1, 6, vec![malformed, unknown_field, field_op::create(T, PROJECT)]));
    both.receive(&change(3, 1, 7, vec![field_op::create(U, TASK), delete(T, false)]));
    both.assert_same();
}

/// The convergence harness against the SQLite view: one copy is a view
/// beside the in-memory fold, and every read checks that both agree.
mod harness {
    use super::*;
    use proptest::prelude::*;
    use support::convergence::{self, Config, Mergeable, Op, OpId, Outcome, independent};

    struct Copy(Both);

    impl Default for Copy {
        fn default() -> Copy {
            Copy(Both::new())
        }
    }

    /// A clone is a new view that receives the same frames.
    impl Clone for Copy {
        fn clone(&self) -> Copy {
            let mut copy = Copy::default();
            for frame in &self.0.frames {
                copy.0.receive(frame);
            }
            copy
        }
    }

    impl std::fmt::Debug for Copy {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Copy")
                .field("frames", &self.0.frames.len())
                .finish()
        }
    }

    impl Mergeable for Copy {
        type Change = Vec<u8>;
        type Read = Vec<Option<ncdb_core::view::Row>>;

        fn apply(&mut self, op: &Op<Vec<u8>>) {
            self.0.receive(&op.body);
        }

        /// The view's tables, byte for byte (RFC 0006, "Why the order of
        /// arrival does not matter").
        fn state_bytes(&self) -> Vec<u8> {
            view_bytes(&self.0.db.connect())
        }

        fn read(&self) -> Self::Read {
            self.0.assert_same();
            [(TASK, T), (TASK, U), (PROJECT, T), (PROJECT, U)]
                .into_iter()
                .map(|(collection, object)| self.0.view.get(SPACE, collection, object).unwrap())
                .collect()
        }

        /// The field types keep every operation; the contract tests of
        /// each type check outcomes.
        fn outcome(&self, _: OpId) -> Outcome {
            Outcome::Visible
        }
    }

    fn operation() -> BoxedStrategy<Operation> {
        let object = prop_oneof![Just(T), Just(U)];
        let words = prop_oneof![Just("a"), Just("b"), Just("c")];
        prop_oneof![
            (object.clone(), prop_oneof![Just(TASK), Just(PROJECT)])
                .prop_map(|(o, c)| field_op::create(o, c)),
            (object.clone(), any::<bool>()).prop_map(|(o, d)| delete(o, d)),
            (object.clone(), words.clone()).prop_map(|(o, w)| title(o, w)),
            (object.clone(), -3..4i64).prop_map(|(o, n)| {
                let value = if n < 0 {
                    Value::Negative(n.unsigned_abs() - 1)
                } else {
                    Value::Unsigned(n.unsigned_abs())
                };
                rank(o, value)
            }),
            (object.clone(), object.clone()).prop_map(|(o, t)| owner(o, t)),
            (object.clone(), words, any::<bool>()).prop_map(|(o, w, a)| tag(o, w, a)),
            object.prop_map(|o| Operation::new(o, TASK.into(), Some(TITLE.into()), 9, None)),
        ]
        .boxed()
    }

    fn changes(author: u8) -> BoxedStrategy<Vec<u8>> {
        (1..6u64, 1..4u64, prop::collection::vec(operation(), 1..4))
            .prop_map(move |(sequence, clock, operations)| {
                change(author, sequence, clock, operations)
            })
            .boxed()
    }

    #[test]
    fn copies_of_the_view_converge_and_read_what_the_in_memory_fold_holds() {
        let config = Config {
            members: 3,
            max_changes_per_member: 3,
            cases: 64,
            ..Config::default()
        };
        let result = convergence::check::<Copy, _>(&config, &independent(changes));
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}
