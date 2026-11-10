//! A view reopened on an existing database checks new changes against the
//! control changes its log already holds (NOC-104; RFC 0006, "Control
//! changes and changes that wait for them"; RFC 0002 D11): the checks the
//! member opens it with know nothing yet, and learn the membership, the
//! keys and the generation graph from the log.

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
use ncdb_core::log::{Appended, State};
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator, who writes the tasks.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

const FIRST: ObjectId = ObjectId([0x11; 16]);
const SECOND: ObjectId = ObjectId([0x22; 16]);

/// B's checks, as the member opens them: they hold no control change.
fn fresh_checks(space: &Space) -> SealChecks {
    SealChecks::new(space.space, space::key(B))
}

/// The control changes that make B a writer and give it the space key.
fn controls(space: &mut Space) -> Vec<Change> {
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    vec![genesis, add, envelope]
}

fn folded() -> State {
    State::Folded { parked: Vec::new() }
}

/// B's view in `db` after the control changes and a task A sealed, closed
/// again.
fn written(db: &TempDb, space: &mut Space) {
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = fresh_checks(space);
    let first = space.sealed(A, vec![field_op::create(FIRST, TASK)]);
    for change in controls(space).iter().chain([&first]) {
        view.receive(change.bytes(), &mut checks).unwrap();
        assert_eq!(view.state(space.space, change.id()), Some(folded()));
    }
}

#[test]
fn a_reopened_view_folds_a_new_change_from_an_existing_member() {
    let db = TempDb::new("reopen");
    let mut space = Space::new(A);
    written(&db, &mut space);

    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = fresh_checks(&space);
    let second = space.sealed(A, vec![field_op::create(SECOND, TASK)]);
    let appended = view.receive(second.bytes(), &mut checks).unwrap();

    assert!(matches!(appended, Appended::New(_)));
    assert_eq!(view.state(space.space, second.id()), Some(folded()));
    let row = view.get(space.space, TASK, SECOND).unwrap();
    assert!(row.is_some_and(|row| row.object.exists()));
}

#[test]
fn a_reopened_view_folds_the_members_own_write() {
    let db = TempDb::new("reopen-own");
    let mut space = Space::new(A);
    written(&db, &mut space);

    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = fresh_checks(&space);
    let own = space.sealed(B, vec![field_op::create(SECOND, TASK)]);
    view.receive(own.bytes(), &mut checks).unwrap();

    assert_eq!(view.state(space.space, own.id()), Some(folded()));
}

/// The SDK learns which spaces a reopened database holds from the view
/// (NOC-108), since a space it was added to has no state of its own.
#[test]
fn a_reopened_view_lists_the_spaces_its_log_holds() {
    let db = TempDb::new("reopen-spaces");
    let mut space = Space::new(A);
    written(&db, &mut space);

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.spaces().collect::<Vec<_>>(), vec![space.space]);
}
