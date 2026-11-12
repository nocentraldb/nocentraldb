//! A model the SDK ships beside the app's lock, the account space's (RFC
//! 0010, "The account space"; NOC-266): the view holds its collections
//! under names that start with `$`, which no app can declare (RFC 0005,
//! "Names"), and folds a space whose genesis names its `model.id` by it.
//! Each space folds by one model: a space of the app's model never reaches
//! the `$` tables, and a space of the shipped model never reaches the
//! app's. A space of any third model parks its content as "another model",
//! as before.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{ParkReason, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::plugins::Registry;
use ncdb_core::view::{Model, View};

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

const M: ObjectId = ObjectId([0x31; 16]);

/// The shipped model of the tests: members and memberships, as the
/// account space's.
fn reserved() -> ModelLock {
    ModelLock::from_json(include_str!("view/reserved.lock.json")).unwrap()
}

/// The registry that ships [`reserved`] beside the app's lock.
fn shipping() -> Registry {
    Registry::default().with_reserved_model(reserved())
}

fn text(object: ObjectId, collection: &str, field: &str, value: &str) -> Operation {
    let write = Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: MemberId([0; 32]),
        stamp: Hlc(0),
        value: Value::Text(value.to_owned()),
        replaces: BTreeSet::new(),
    };
    field_op::encode(
        &BasicValue { kind: Kind::Text },
        &write,
        object,
        collection,
        field,
    )
}

/// A member: an object of `members` with its id.
fn member(collection: &str) -> Vec<Operation> {
    vec![
        field_op::create(M, collection),
        text(M, collection, "memberId", "ab"),
    ]
}

/// B's checks for a space, as the member opens them.
fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

/// A space of `model` that A creates, adds B to with the space key, and
/// writes `operations` into: its changes, in order, and the last one.
fn written(model: [u8; 16], operations: Vec<Operation>) -> (SpaceId, Vec<Change>, Change) {
    let mut space = Space::with_model(A, [3; 16], model);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let content = space.sealed(A, operations);
    let changes = vec![genesis, add, envelope, content.clone()];
    (space.space, changes, content)
}

fn receive(view: &mut View, space: SpaceId, changes: &[Change]) {
    let mut checks = checks_for(space);
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
}

#[test]
fn a_space_of_the_shipped_model_folds_into_its_dollar_tables() {
    let mut view = View::open_in_memory_with_plugins(&lock(), &shipping()).unwrap();
    let (space, changes, content) = written(reserved().model.id.0, member("members"));
    receive(&mut view, space, &changes);

    assert!(matches!(
        view.state(space, content.id()),
        Some(State::Folded { .. })
    ));
    let row = view.get(space, "$members", M).unwrap().expect("the member");
    assert!(row.fields.contains_key("memberId"));
}

#[test]
fn the_model_holds_the_shipped_collections_under_dollar_names() {
    let model = Model::from_lock_with(&lock(), &shipping()).unwrap();
    assert!(model.collections.contains_key("$members"));
    assert!(model.collections.contains_key("$memberships"));
    assert!(!model.collections.contains_key("members"));
    assert!(model.collections.contains_key(TASK));
    // A link of the shipped model targets the shipped collection.
    let link = model.field("$memberships", "member");
    assert!(
        format!("{link:?}").contains("$members"),
        "the link targets $members: {link:?}"
    );
    // The app's model alone hashes as before.
    let alone = Model::from_lock(&lock()).unwrap();
    assert_ne!(model.hash(), alone.hash());
    assert_eq!(
        alone,
        Model::from_lock_with(&lock(), &Registry::default()).unwrap()
    );
}

#[test]
fn a_space_of_the_apps_model_never_reaches_the_dollar_tables() {
    let mut view = View::open_in_memory_with_plugins(&lock(), &shipping()).unwrap();
    let (space, changes, _) = written(lock().model.id.0, member("$members"));
    receive(&mut view, space, &changes);

    assert!(view.get(space, "$members", M).unwrap().is_none());
}

#[test]
fn a_space_of_the_shipped_model_never_reaches_the_apps_tables() {
    let mut view = View::open_in_memory_with_plugins(&lock(), &shipping()).unwrap();
    let operations = vec![
        field_op::create(M, TASK),
        text(M, TASK, TITLE, "not a task"),
    ];
    let (space, changes, _) = written(reserved().model.id.0, operations);
    receive(&mut view, space, &changes);

    assert!(view.get(space, TASK, M).unwrap().is_none());
}

#[test]
fn a_space_of_a_third_model_still_parks_as_another_model() {
    let mut view = View::open_in_memory_with_plugins(&lock(), &shipping()).unwrap();
    let mut third = reserved().model.id.0;
    third[0] ^= 0xff;
    let (space, changes, content) = written(third, member("members"));
    receive(&mut view, space, &changes);

    assert_eq!(
        view.state(space, content.id()),
        Some(State::Parked(ParkReason::AnotherModel))
    );
}

#[test]
fn without_the_shipped_model_its_space_parks_as_another_model() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let (space, changes, content) = written(reserved().model.id.0, member("members"));
    receive(&mut view, space, &changes);

    assert_eq!(
        view.state(space, content.id()),
        Some(State::Parked(ParkReason::AnotherModel))
    );
}

#[test]
fn a_view_opened_again_with_the_shipped_model_folds_its_spaces() {
    let db = TempDb::new("reserved-model");
    let (space, changes, content) = written(reserved().model.id.0, member("members"));
    let mut view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    receive(&mut view, space, &changes);
    drop(view);

    let view = View::open_with_plugins(&db.0, &lock(), &shipping(), checks_for).unwrap();

    assert!(matches!(
        view.state(space, content.id()),
        Some(State::Folded { .. })
    ));
    assert!(view.get(space, "$members", M).unwrap().is_some());
}

#[test]
fn a_shipped_model_of_the_apps_own_id_is_refused() {
    let mut same = reserved();
    same.model.id = lock().model.id;
    let registry = Registry::default().with_reserved_model(same);
    assert!(Model::from_lock_with(&lock(), &registry).is_err());
}
