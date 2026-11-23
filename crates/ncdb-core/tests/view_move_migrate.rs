//! A move and its take-back that an older model parked (RFC 0016, "What
//! the members of the old space keep"; RFC 0006, "Rebuilding the view"):
//! when an additive change of the model folds them in place, the object's
//! fields written while it was moved are folded again after the take-back,
//! and the view equals one written with the new model from the start.

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
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator, who writes.
const A: u8 = 1;
/// The member whose view this is.
const B: u8 = 2;

const P: ObjectId = ObjectId([0x33; 16]);

/// [`lock`] one version before, without the projects and the tasks' link
/// to them.
fn without_projects() -> ModelLock {
    let mut lock = lock();
    lock.collections.remove(PROJECT).unwrap();
    lock.collections
        .get_mut(TASK)
        .unwrap()
        .fields
        .remove(OWNER)
        .unwrap();
    lock.model.version -= 1;
    lock
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

fn name(value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    field_op::encode(&text, &write(Value::Text(value.into())), P, PROJECT, NAME)
}

fn move_to(value: Value) -> Operation {
    field_op::move_object(P, PROJECT, &write(value))
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

/// A project created, named, moved, renamed while moved, and moved back.
fn changes() -> (SpaceId, Vec<Change>) {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let mut all = vec![genesis, add, envelope];
    for operations in [
        vec![field_op::create(P, PROJECT), name("first")],
        vec![move_to(Value::Bytes(vec![0x5a; 16]))],
        vec![name("while moved")],
        vec![move_to(Value::Null)],
    ] {
        all.push(space.sealed(A, operations));
    }
    (space.space, all)
}

fn receive_all(db: &TempDb, lock: &ModelLock, space: SpaceId, changes: &[Change]) -> View {
    let mut view = View::open_with(&db.0, lock, checks_for).unwrap();
    let mut checks = checks_for(space);
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    view
}

#[test]
fn a_take_back_folded_by_a_migration_folds_the_fields_again() {
    let (space, changes) = changes();
    let migrated = TempDb::new("move-migrate");
    drop(receive_all(&migrated, &without_projects(), space, &changes));
    let view = View::open_with(&migrated.0, &lock(), checks_for).unwrap();

    let fresh = TempDb::new("move-fresh");
    let expected = receive_all(&fresh, &lock(), space, &changes);

    let row = view.get(space, PROJECT, P).unwrap().unwrap();
    assert_eq!(row.object.moved_to(), None);
    assert_eq!(row, expected.get(space, PROJECT, P).unwrap().unwrap());
    assert_eq!(
        row.fields.len(),
        1,
        "the name written while moved is folded"
    );
}
