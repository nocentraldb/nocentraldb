//! What a view keeps of its relay across a reopen (NOC-145; RFC 0007,
//! "Restores and the relay epoch"): the relay epoch its relay positions
//! belong to, and each space's cursor, derived from those positions as the
//! highest position up to which the log holds every one.

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
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

/// B's view after the control changes that make B a writer and two tasks:
/// the changes in local order.
fn received(view: &mut View, space: &mut Space) -> Vec<Change> {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let first = space.sealed(A, vec![field_op::create(ObjectId([1; 16]), TASK)]);
    let second = space.sealed(B, vec![field_op::create(ObjectId([2; 16]), TASK)]);
    let changes = vec![genesis, add, envelope, first, second];
    for change in &changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    changes
}

#[test]
fn a_new_view_knows_no_relay_epoch() {
    let view = View::open_in_memory(&lock()).unwrap();
    assert_eq!(view.relay_epoch().unwrap(), None);
}

#[test]
fn the_relay_epoch_is_kept_across_a_reopen() {
    let db = TempDb::new("relay-epoch");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.set_relay_epoch([7; 16]).unwrap();
    view.set_relay_epoch([9; 16]).unwrap();
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.relay_epoch().unwrap(), Some([9; 16]));
}

#[test]
fn the_cursor_is_the_highest_position_without_a_gap_below_it() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    assert_eq!(view.relay_cursor(space.space).unwrap(), 0);

    view.confirm(space.space, changes[0].id(), 1).unwrap();
    view.confirm(space.space, changes[1].id(), 2).unwrap();
    // B pushed its own change and the relay gave it position 5, past
    // positions B has not pulled yet.
    view.confirm(space.space, changes[4].id(), 5).unwrap();
    assert_eq!(view.relay_cursor(space.space).unwrap(), 2);

    view.confirm(space.space, changes[2].id(), 3).unwrap();
    view.confirm(space.space, changes[3].id(), 4).unwrap();
    assert_eq!(view.relay_cursor(space.space).unwrap(), 5);
}

#[test]
fn a_cursor_counts_only_its_own_space_and_goes_with_the_positions() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    view.confirm(space.space, changes[0].id(), 1).unwrap();
    let other = Space::with_nonce(A, [9; 16]);
    assert_eq!(view.relay_cursor(other.space).unwrap(), 0);

    view.forget_relay_positions().unwrap();
    assert_eq!(view.relay_cursor(space.space).unwrap(), 0);
}
