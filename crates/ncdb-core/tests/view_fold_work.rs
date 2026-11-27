//! What folding one change of many objects costs the view's tables
//! (NOC-542). A write of 1,000 objects in a browser got slower after
//! 0.1.0-beta.2: the fold prepared each of its SQL statements anew for
//! every operation, and read the object's state from `objects` once more
//! for every field operation, to learn whether the object is a moved
//! trace (RFC 0016). A fold prepares each of its statements once per
//! change, and reads each object's state at most once.
//!
//! The counts come from `View::fold_work`, so they do not depend on the
//! machine's speed.

mod support;

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
use ncdb_core::change::Operation;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::log::State;
use ncdb_core::membership::Role;
use ncdb_core::view::{FoldWork, View};

use space::Space;
use view_support::*;

/// The member who writes the tasks: the space's creator.
const WRITER: u8 = 1;

/// The member whose view this is.
const ME: u8 = 2;

/// The most statements one fold of a change may prepare, whatever the
/// number of its objects.
const STATEMENTS: usize = 24;

fn write(value: Value) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: space::id(WRITER),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn object(n: u64) -> ObjectId {
    let mut id = [0; 16];
    id[..8].copy_from_slice(&n.to_be_bytes());
    ObjectId(id)
}

/// Task `n` with a title and a rank, as a write of the SDK creates it.
fn task(n: u64) -> Vec<Operation> {
    let text = BasicValue { kind: Kind::Text };
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    vec![
        field_op::create(object(n), TASK),
        field_op::encode(
            &text,
            &write(Value::Text(format!("task {n}"))),
            object(n),
            TASK,
            TITLE,
        ),
        field_op::encode(
            &integer,
            &write(Value::Unsigned(n % 5)),
            object(n),
            TASK,
            RANK,
        ),
    ]
}

/// What folding one change of `tasks` new tasks did, into a view that
/// already holds a change of `tasks` others.
fn work_of(tasks: u64) -> FoldWork {
    let mut space = Space::new(WRITER);
    let genesis = space.genesis.clone();
    let add = space.add(WRITER, ME, Role::Writer, &[&genesis]);
    let envelope = space.envelope(WRITER, ME, &[&add]);
    let mut checks = SealChecks::new(space.space, space::key(ME));
    let mut view = View::open_in_memory(&lock()).unwrap();
    for control in [&genesis, &add, &envelope] {
        view.receive(control.bytes(), &mut checks).unwrap();
    }
    for range in [0..tasks, tasks..2 * tasks] {
        let change = space.sealed(WRITER, range.flat_map(task).collect());
        view.receive(change.bytes(), &mut checks).unwrap();
        assert_eq!(
            view.log(space.space).unwrap().state(change.id()),
            Some(&State::Folded { parked: vec![] }),
            "the tasks are folded"
        );
    }
    view.fold_work()
}

#[test]
fn a_fold_prepares_each_statement_once_whatever_the_number_of_objects() {
    let few = work_of(10);
    let many = work_of(500);
    assert!(
        many.prepared <= STATEMENTS,
        "a change of 500 tasks prepared {} statements, of 10 tasks {}",
        many.prepared,
        few.prepared
    );
    assert_eq!(many.prepared, few.prepared);
}

#[test]
fn a_fold_reads_each_objects_state_at_most_once() {
    let work = work_of(500);
    assert!(
        work.object_reads <= 500,
        "a change of 500 tasks read an object's state {} times",
        work.object_reads
    );
}
