//! An old client and a day field (RFC 0005, "Data from a newer model";
//! RFC 0004, "Days and points in time"): a client whose lock does not have
//! the field keeps the change, folds its other operations and parks the
//! write of the day as an unknown field. After its upgrade the view adds
//! the field's column and folds the parked write, as a client with the new
//! lock all along would. Nothing it held before changes.

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
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{Field, Kind as LockKind, ModelLock};
use ncdb_core::view::{FieldState, View};

use space::Space;
use view_support::*;

/// The space's creator, who writes the task.
const A: u8 = 1;
/// The member whose view this is.
const B: u8 = 2;
/// The new field: the day a task was done.
const DONE_ON: &str = "doneOn";

/// [`lock`] one version later, with the tasks' day `doneOn`.
fn with_done_on() -> ModelLock {
    let mut lock = lock();
    let version = lock.model.version + 1;
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.insert(
        DONE_ON.into(),
        Field {
            kind: LockKind::Date,
            rule: 1,
            since: version,
            personal: None,
            description: None,
            deprecated: None,
            vocabulary: Default::default(),
        },
    );
    lock.model.version = version;
    lock.model.hash = lock.compute_hash();
    lock
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

fn write(value: Value) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: space::id(A),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

/// A task with a title, done on 2026-11-21, in one change of A's.
fn task(object: ObjectId) -> Vec<Operation> {
    let title = BasicValue { kind: Kind::Text };
    let day = BasicValue { kind: Kind::Date };
    vec![
        field_op::create(object, TASK),
        field_op::encode(&title, &write(Value::Text("t".into())), object, TASK, TITLE),
        field_op::encode(&day, &write(Value::Unsigned(20_778)), object, TASK, DONE_ON),
    ]
}

/// The changes of a space in which B is a writer, the last one the task.
fn changes(object: ObjectId) -> (SpaceId, Vec<Change>) {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let sealed = space.sealed(A, task(object));
    (space.space, vec![genesis, add, envelope, sealed])
}

#[test]
fn an_old_client_parks_the_day_and_folds_it_after_its_upgrade() {
    let db = TempDb::new("dates-old-client");
    let object = ObjectId([7; 16]);
    let (space, changes) = changes(object);
    {
        let mut old = View::open_with(&db.0, &lock(), checks_for).unwrap();
        let mut checks = checks_for(space);
        for change in &changes {
            old.receive(change.bytes(), &mut checks).unwrap();
        }
        let task = changes.last().unwrap();
        assert_eq!(
            old.state(space, task.id()),
            Some(State::Folded {
                parked: vec![ParkedOperation {
                    position: 2,
                    reason: OperationParkReason::UnknownField,
                }],
            })
        );
        let row = old.get(space, TASK, object).unwrap().unwrap();
        assert!(row.fields.contains_key(TITLE));
        assert!(!row.fields.contains_key(DONE_ON));
    }

    let upgraded = View::open_with(&db.0, &with_done_on(), checks_for).unwrap();
    let task = changes.last().unwrap();
    assert_eq!(
        upgraded.state(space, task.id()),
        Some(State::Folded { parked: Vec::new() })
    );
    let row = upgraded.get(space, TASK, object).unwrap().unwrap();
    let Some(FieldState::Basic(state)) = row.fields.get(DONE_ON) else {
        panic!(
            "doneOn is folded as a basic value: {:?}",
            row.fields.get(DONE_ON)
        );
    };
    let read = BasicValue { kind: Kind::Date }.read(state);
    assert_eq!(
        read.value.map(|entry| entry.value),
        Some(Value::Unsigned(20_778))
    );
}
