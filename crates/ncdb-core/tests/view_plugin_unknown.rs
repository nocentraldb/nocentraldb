//! A plugin field whose type has no code (RFC 0011, "A type that is not
//! registered"; Carsten, 2026-11-12, NOC-232): the view opens anyway and
//! parks the field's operations with the reason "unknown type", which RFC
//! 0004 and RFC 0005 name, apart from an operation on a field the lock
//! does not name, which stays "unknown field". The change is kept, and the
//! rest of it is folded.

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
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{self, ModelLock};
use ncdb_core::view::View;

use space::Space;
use view_support::*;

const A: u8 = 1;
const B: u8 = 2;

const T: ObjectId = ObjectId([0x11; 16]);

/// The plugin field of the task, and a field no lock names.
const PRICE: &str = "price";
const COLOUR: &str = "colour";

const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// The view tests' lock, with a task field of the plugin type
/// `acme/money`, rule 1, which no code implements here.
fn with_plugin_field() -> ModelLock {
    let mut lock = lock();
    let mut field = lock.field(TASK, RANK).unwrap().clone();
    field.kind = model_lock::Kind::Plugin {
        type_id: "acme/money".into(),
        config: None,
    };
    lock.plugins
        .insert("acme".into(), model_lock::Plugin::default());
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.insert(PRICE.into(), field);
    lock
}

/// An integer write to `field` of the task: a plugin's operations look
/// like any other in the change, so this stands for one.
fn write(field: &str, value: u64) -> Operation {
    let write = Write {
        id: ANY,
        author: space::id(A),
        stamp: Hlc(0),
        value: Value::Unsigned(value),
        replaces: BTreeSet::new(),
    };
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    field_op::encode(&integer, &write, T, TASK, field)
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

#[test]
fn an_operation_on_a_plugin_field_without_code_parks_as_unknown_type() {
    let db = TempDb::new("plugin-unknown-type");
    let mut view = View::open_with(&db.0, &with_plugin_field(), checks_for).unwrap();
    let mut space = Space::new(A);
    let mut checks = checks_for(space.space);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let task = space.sealed(
        A,
        vec![
            field_op::create(T, TASK),
            write(PRICE, 7),
            write(COLOUR, 1),
            write(RANK, 3),
        ],
    );
    for change in [&genesis, &add, &envelope, &task] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    assert_eq!(
        view.state(space.space, task.id()),
        Some(State::Folded {
            parked: vec![
                ParkedOperation {
                    position: 1,
                    reason: OperationParkReason::UnknownType,
                },
                ParkedOperation {
                    position: 2,
                    reason: OperationParkReason::UnknownField,
                },
            ],
        })
    );
    let row = view.get(space.space, TASK, T).unwrap().unwrap();
    assert!(row.fields.contains_key(RANK));
    assert!(!row.fields.contains_key(PRICE));
}
