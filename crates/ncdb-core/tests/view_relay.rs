//! The relay's answers in the member's log (RFC 0002, "The log on disk";
//! RFC 0007, "Island mode" and "Restores and the relay epoch"): the view
//! records the relay position of each change and the relay's refusal of
//! one, hands out the changes the relay has not confirmed in local order,
//! and forgets every position when the relay's epoch changes. A reopened
//! view reads them back.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::RelayRefusal;
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

/// B's view after the control changes that make B a writer and three
/// tasks, A's, B's and A's: the changes in local order.
fn received(view: &mut View, space: &mut Space) -> Vec<Change> {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let first = space.sealed(A, vec![field_op::create(ObjectId([1; 16]), TASK)]);
    let second = space.sealed(B, vec![field_op::create(ObjectId([2; 16]), TASK)]);
    let third = space.sealed(A, vec![field_op::create(ObjectId([3; 16]), TASK)]);
    let changes = vec![genesis, add, envelope, first, second, third];
    for change in &changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    changes
}

fn ids(changes: &[Change]) -> Vec<[u8; 32]> {
    changes.iter().map(|change| change.id().0).collect()
}

#[test]
fn every_change_is_unconfirmed_until_the_relay_gives_it_a_position() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);

    assert_eq!(ids(&view.unconfirmed(space.space).unwrap()), ids(&changes));
    assert_eq!(view.relay_position(space.space, changes[3].id()), None);
}

#[test]
fn a_confirmed_change_leaves_the_unconfirmed_ones_in_local_order() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);

    view.confirm(space.space, changes[0].id(), 1).unwrap();
    view.confirm(space.space, changes[4].id(), 2).unwrap();

    let expected = [&changes[1..4], &changes[5..]].concat();
    assert_eq!(ids(&view.unconfirmed(space.space).unwrap()), ids(&expected));
    assert_eq!(view.relay_position(space.space, changes[4].id()), Some(2));
}

#[test]
fn a_refused_change_is_not_unconfirmed_and_is_listed_with_its_code() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);

    view.refuse(space.space, changes[4].id(), "author_not_member")
        .unwrap();

    let unconfirmed = view.unconfirmed(space.space).unwrap();
    assert!(!ids(&unconfirmed).contains(&changes[4].id().0));
    let refused = view.refused(space.space).unwrap();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].0.id(), changes[4].id());
    assert_eq!(refused[0].1, RelayRefusal("author_not_member".into()));
    assert_eq!(
        view.relay_refusal(space.space, changes[4].id()),
        Some(RelayRefusal("author_not_member".into()))
    );
}

#[test]
fn a_new_epoch_makes_every_position_unconfirmed_again_and_keeps_refusals() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    for (change, position) in changes.iter().zip(1..).take(4) {
        view.confirm(space.space, change.id(), position).unwrap();
    }
    view.refuse(space.space, changes[4].id(), "bad_signature")
        .unwrap();

    view.forget_relay_positions().unwrap();

    let expected = [&changes[..4], &changes[5..]].concat();
    assert_eq!(ids(&view.unconfirmed(space.space).unwrap()), ids(&expected));
    assert_eq!(view.relay_position(space.space, changes[0].id()), None);
    assert_eq!(view.refused(space.space).unwrap().len(), 1);
}

#[test]
fn a_reopened_view_keeps_positions_and_refusals() {
    let db = TempDb::new("relay");
    let mut space = Space::new(A);
    let changes = {
        let mut view = View::open(&db.0, &lock()).unwrap();
        let changes = received(&mut view, &mut space);
        view.confirm(space.space, changes[0].id(), 1).unwrap();
        view.refuse(space.space, changes[1].id(), "membership_rule")
            .unwrap();
        changes
    };

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.relay_position(space.space, changes[0].id()), Some(1));
    assert_eq!(
        view.relay_refusal(space.space, changes[1].id()),
        Some(RelayRefusal("membership_rule".into()))
    );
    assert_eq!(
        ids(&view.unconfirmed(space.space).unwrap()),
        ids(&changes[2..])
    );
}

#[test]
fn a_space_the_view_does_not_hold_has_nothing_unconfirmed() {
    let view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);

    assert!(view.unconfirmed(space.space).unwrap().is_empty());
    assert!(view.refused(space.space).unwrap().is_empty());
}
