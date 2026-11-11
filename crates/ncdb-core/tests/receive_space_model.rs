//! A space whose genesis names another `model.id` than the member's lock
//! is never folded (RFC 0009, "Spaces of another model"; RFC 0005, "Two
//! checks: at build and on the device"; NOC-57). Its control changes fold,
//! so the member can name the space's model. Each sealed change stops at
//! step 6 as "another model", before anything of it reaches the view,
//! whatever its own payload key 64 says. A space of the lock's model folds
//! as before. A lock can never take another `model.id` (RFC 0005, "Two
//! checks"), so no lock change folds such a space: an additive lock that
//! folds the view again parks it again.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::ids::SpaceId;
use ncdb_core::log::{ParkReason, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

const TASK_ID: ObjectId = ObjectId([0x11; 16]);

/// B's view after A creates a space of `model`, adds B with the space key,
/// and writes one task. Returns the view, the space, B's envelope and the
/// task.
fn received(model: [u8; 16]) -> (View, Space, Change, Change) {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::with_model(A, [3; 16], model);
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let task = space.sealed(A, vec![field_op::create(TASK_ID, TASK)]);
    for change in [&genesis, &add, &envelope, &task] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (view, space, envelope, task)
}

#[test]
fn a_space_of_another_model_folds_its_membership_but_none_of_its_content() {
    let mut other = lock().model.id.0;
    other[0] ^= 0xff;
    let (view, space, envelope, task) = received(other);

    assert!(matches!(
        view.state(space.space, envelope.id()),
        Some(State::Folded { .. })
    ));
    assert_eq!(
        view.state(space.space, task.id()),
        Some(State::Parked(ParkReason::AnotherModel))
    );
    assert!(view.get(space.space, TASK, TASK_ID).unwrap().is_none());
    assert_eq!(
        view.state_counts(space.space)
            .parked
            .get(&ParkReason::AnotherModel),
        Some(&1)
    );
}

#[test]
fn a_space_of_the_locks_model_folds_its_content() {
    let (view, space, _, task) = received(lock().model.id.0);

    assert!(matches!(
        view.state(space.space, task.id()),
        Some(State::Folded { .. })
    ));
    assert!(view.get(space.space, TASK, TASK_ID).unwrap().is_some());
}

/// [`lock`] one version before, without the tasks' rank and the indexes on
/// it: the lock it is an additive successor of.
fn older() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

/// B's checks, as the member opens them: they hold no control change.
fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

#[test]
fn an_additive_lock_change_folds_the_view_again_and_parks_the_space_of_another_model_again() {
    let db = TempDb::new("space-model-additive");
    let mut other = lock().model.id.0;
    other[0] ^= 0xff;
    let mut space = Space::with_model(A, [3; 16], other);
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    let mut checks = checks_for(space.space);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let task = space.sealed(A, vec![field_op::create(TASK_ID, TASK)]);
    for change in [&genesis, &add, &envelope, &task] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();

    assert_eq!(
        view.state(space.space, task.id()),
        Some(State::Parked(ParkReason::AnotherModel))
    );
    assert!(view.get(space.space, TASK, TASK_ID).unwrap().is_none());
}
