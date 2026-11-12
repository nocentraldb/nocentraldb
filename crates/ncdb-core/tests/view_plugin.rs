//! A plugin field in the view (RFC 0011, "The field type a plugin
//! implements" and "A type that is not registered"; Carsten, 2026-11-12,
//! NOC-232), with the test plugin `test/counter`: the core folds its
//! operations through the plugin's code over the stored bytes, parks a
//! number below 64 itself, drops argument keys from 64 up, and lets the
//! plugin park what it cannot read. A rule or a `config` the registry has
//! no code for parks as unknown type.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{self, ModelLock};
use ncdb_core::plugins::Registry;
use ncdb_core::view::{Field, FieldState, View};
use ncdb_plugin::Data;

use space::Space;
use view_support::*;

const A: u8 = 1;
const B: u8 = 2;

const T: ObjectId = ObjectId([0x11; 16]);

/// The task's counter field.
const VOTES: &str = "votes";

/// The view tests' lock, with a task field of `test/counter` at `rule`,
/// with `config`.
fn lock_with(rule: u64, config: Option<serde_json::Value>) -> ModelLock {
    let mut lock = lock();
    let mut field = lock.field(TASK, RANK).unwrap().clone();
    field.kind = model_lock::Kind::Plugin {
        type_id: "test/counter".into(),
        config,
    };
    field.rule = rule;
    lock.plugins
        .insert("test".into(), model_lock::Plugin::default());
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.insert(VOTES.into(), field);
    lock
}

fn registry() -> Registry {
    Registry::new([ncdb_test_counter::plugin()]).unwrap()
}

/// An operation with `number` and these arguments on the task's votes.
fn op(number: u64, arguments: Value) -> Operation {
    Operation::new(
        T,
        TASK.to_owned(),
        Some(VOTES.to_owned()),
        number,
        Some(arguments),
    )
}

/// The arguments `{key: n, ...}`.
fn args(entries: &[(u64, u64)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(key, n)| (Key::Unsigned(*key), Value::Unsigned(*n)))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

/// B's view of a space where A, its creator, made B a writer and wrote
/// `operations` in one change after creating the task; the space and the
/// change.
fn folded(view: &mut View, operations: Vec<Operation>) -> (SpaceId, Change) {
    let mut space = Space::new(A);
    let mut checks = checks_for(space.space);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let mut all = vec![field_op::create(T, TASK)];
    all.extend(operations);
    let task = space.sealed(A, all);
    for change in [&genesis, &add, &envelope, &task] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (space.space, task)
}

/// What the plugin reads from the task's votes in `view`.
fn votes(view: &View, space: SpaceId) -> Option<Data> {
    let row = view.get(space, TASK, T).unwrap()?;
    let state = row.fields.get(VOTES)?;
    let Some(Field::Plugin(field)) = view.model().field(TASK, VOTES) else {
        panic!("votes is no plugin field in the view's model");
    };
    let FieldState::Plugin(bytes) = state else {
        panic!("votes holds no plugin state");
    };
    field.read(bytes)
}

fn parked(position: u64, reason: OperationParkReason) -> ParkedOperation {
    ParkedOperation { position, reason }
}

#[test]
fn the_core_folds_a_plugin_field_through_its_code() {
    let mut view = View::open_in_memory_with_plugins(&lock_with(1, None), &registry()).unwrap();
    let (space, task) = folded(
        &mut view,
        vec![
            op(64, args(&[(1, 2)])),
            op(64, args(&[(1, 3)])),
            // A key from 64 up is dropped before the plugin sees it.
            op(64, args(&[(1, 5), (70, 9)])),
        ],
    );
    assert_eq!(
        view.state(space, task.id()),
        Some(State::Folded { parked: vec![] })
    );
    assert_eq!(votes(&view, space), Some(Data::Integer(10)));
    assert!(!view.model().field(TASK, VOTES).unwrap().has_column());
}

#[test]
fn a_number_below_64_parks_in_the_core_and_the_plugin_parks_what_it_cannot_read() {
    let mut view = View::open_in_memory_with_plugins(&lock_with(1, None), &registry()).unwrap();
    let (space, task) = folded(
        &mut view,
        vec![
            // Set (1) is a built-in number: the core parks it.
            op(1, args(&[(1, 4)])),
            // 65 is no operation of the counter.
            op(65, args(&[(1, 4)])),
            // Key 2 is no argument of an add.
            op(64, args(&[(1, 4), (2, 1)])),
            // No n.
            op(64, args(&[])),
            // Arguments that are no map.
            op(64, Value::Unsigned(4)),
            op(64, args(&[(1, 7)])),
        ],
    );
    assert_eq!(
        view.state(space, task.id()),
        Some(State::Folded {
            parked: vec![
                parked(1, OperationParkReason::UnknownOperation),
                parked(2, OperationParkReason::UnknownOperation),
                parked(3, OperationParkReason::UnknownArgument),
                parked(4, OperationParkReason::MalformedArguments),
                parked(5, OperationParkReason::MalformedArguments),
            ],
        })
    );
    assert_eq!(votes(&view, space), Some(Data::Integer(7)));
}

#[test]
fn a_rule_the_registry_does_not_hold_parks_as_unknown_type() {
    let mut view = View::open_in_memory_with_plugins(&lock_with(2, None), &registry()).unwrap();
    let (space, task) = folded(&mut view, vec![op(64, args(&[(1, 2)]))]);
    assert_eq!(
        view.state(space, task.id()),
        Some(State::Folded {
            parked: vec![parked(1, OperationParkReason::UnknownType)],
        })
    );
    assert!(view.model().unregistered(TASK, VOTES));
}

#[test]
fn a_config_the_plugin_refuses_parks_as_unknown_type() {
    let config = serde_json::json!({ "step": 2 });
    let mut view =
        View::open_in_memory_with_plugins(&lock_with(1, Some(config)), &registry()).unwrap();
    let (space, task) = folded(&mut view, vec![op(64, args(&[(1, 2)]))]);
    assert_eq!(
        view.state(space, task.id()),
        Some(State::Folded {
            parked: vec![parked(1, OperationParkReason::UnknownType)],
        })
    );
}

#[test]
fn without_a_registry_the_plugin_field_parks_as_before() {
    let mut view = View::open_in_memory(&lock_with(1, None)).unwrap();
    let (space, task) = folded(&mut view, vec![op(64, args(&[(1, 2)]))]);
    assert_eq!(
        view.state(space, task.id()),
        Some(State::Folded {
            parked: vec![parked(1, OperationParkReason::UnknownType)],
        })
    );
}

#[test]
fn the_stored_state_is_read_back_after_a_reopen_and_folds_on() {
    let db = TempDb::new("plugin-reopen");
    let lock = lock_with(1, None);
    let mut view = View::open_with_plugins(&db.0, &lock, &registry(), checks_for).unwrap();
    let (space, _) = folded(&mut view, vec![op(64, args(&[(1, 2)]))]);
    drop(view);
    let view = View::open_with_plugins(&db.0, &lock, &registry(), checks_for).unwrap();
    assert_eq!(votes(&view, space), Some(Data::Integer(2)));
}

#[test]
fn a_model_with_the_plugin_field_registered_hashes_apart_from_one_without() {
    let lock = lock_with(1, None);
    let registered = ncdb_core::view::Model::from_lock_with(&lock, &registry()).unwrap();
    let unregistered = ncdb_core::view::Model::from_lock(&lock).unwrap();
    assert_ne!(registered.hash(), unregistered.hash());
}
