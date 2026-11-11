//! Clearing the relay's refusal of a change (RFC 0008, "How the relay
//! applies the rules"; RFC 0002,
//! "A change the relay refuses after the member folded it"): when the
//! member's own fold newly accepts a change the relay refused, the member
//! pushes it to that relay once more. `View::clear_refusal` makes it
//! unconfirmed again, so `View::unconfirmed` hands it out, and the relay
//! counts follow. The change's state does not change, and a reopened view
//! keeps the refusal cleared.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::RelayCounts;
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

/// B's view after the control changes that make B a writer and two of B's
/// tasks: the changes in local order.
fn received(view: &mut View, space: &mut Space) -> Vec<Change> {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let first = space.sealed(B, vec![field_op::create(ObjectId([1; 16]), TASK)]);
    let second = space.sealed(B, vec![field_op::create(ObjectId([2; 16]), TASK)]);
    let changes = vec![genesis, add, envelope, first, second];
    for change in &changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    changes
}

#[test]
fn a_cleared_refusal_makes_the_change_unconfirmed_again() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    let at = space.space;
    for (position, change) in changes[..3].iter().enumerate() {
        view.confirm(at, change.id(), position as u64 + 1).unwrap();
    }
    view.refuse(at, changes[3].id(), "author_not_member")
        .unwrap();
    let state = view.state(at, changes[3].id());

    view.clear_refusal(at, changes[3].id()).unwrap();

    assert_eq!(view.relay_refusal(at, changes[3].id()), None);
    assert!(view.refused(at).unwrap().is_empty());
    let unconfirmed: Vec<_> = view
        .unconfirmed(at)
        .unwrap()
        .iter()
        .map(Change::id)
        .collect();
    assert_eq!(unconfirmed, [changes[3].id(), changes[4].id()]);
    assert_eq!(
        view.relay_counts(at, space::id(B)),
        RelayCounts {
            unconfirmed: 2,
            refused: 0
        }
    );
    assert_eq!(view.state(at, changes[3].id()), state, "the state changed");
}

#[test]
fn clearing_a_refusal_of_a_confirmed_change_leaves_it_confirmed() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    let at = space.space;
    view.refuse(at, changes[3].id(), "author_not_member")
        .unwrap();
    view.confirm(at, changes[3].id(), 4).unwrap();

    view.clear_refusal(at, changes[3].id()).unwrap();

    assert!(
        view.unconfirmed(at)
            .unwrap()
            .iter()
            .all(|c| c.id() != changes[3].id())
    );
    assert_eq!(view.relay_counts(at, space::id(B)).refused, 0);
    assert_eq!(view.relay_counts(at, space::id(B)).unconfirmed, 1);
}

#[test]
fn clearing_twice_or_a_change_never_refused_changes_nothing() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    let at = space.space;
    view.refuse(at, changes[3].id(), "author_not_member")
        .unwrap();
    view.clear_refusal(at, changes[3].id()).unwrap();
    let counts = view.relay_counts(at, space::id(B));

    view.clear_refusal(at, changes[3].id()).unwrap();
    view.clear_refusal(at, changes[4].id()).unwrap();

    assert_eq!(view.relay_counts(at, space::id(B)), counts);
    assert_eq!(counts.unconfirmed, 2);
}

#[test]
fn a_reopened_view_keeps_the_refusal_cleared() {
    let db = TempDb::new("relay-clear");
    let mut space = Space::new(A);
    let changes = {
        let mut view = View::open(&db.0, &lock()).unwrap();
        let changes = received(&mut view, &mut space);
        view.refuse(space.space, changes[3].id(), "author_not_member")
            .unwrap();
        view.refuse(space.space, changes[4].id(), "author_not_member")
            .unwrap();
        view.clear_refusal(space.space, changes[3].id()).unwrap();
        changes
    };

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.relay_refusal(space.space, changes[3].id()), None);
    assert!(view.relay_refusal(space.space, changes[4].id()).is_some());
    assert_eq!(
        view.relay_counts(space.space, space::id(B)),
        RelayCounts {
            unconfirmed: 1,
            refused: 1
        }
    );
}
