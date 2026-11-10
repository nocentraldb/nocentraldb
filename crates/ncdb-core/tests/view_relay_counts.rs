//! The relay counts of each author (NOC-147): how many of an author's
//! changes the relay has neither confirmed nor refused, and how many it
//! refused. The view keeps them current as changes are received, confirmed
//! and refused, and as the relay's epoch changes, so a member's sync status
//! reads them without decoding its log. After every step they equal a full
//! recount of `View::unconfirmed` and `View::refused`, and a reopened view
//! reads them back.
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
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::log::RelayCounts;
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;
/// A member that wrote nothing.
const C: u8 = 3;

/// B's view after the control changes that make B a writer and four
/// tasks, A's, B's, A's and B's: the changes in local order.
fn received(view: &mut View, space: &mut Space) -> Vec<Change> {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let tasks = [(A, 1), (B, 2), (A, 3), (B, 4)]
        .map(|(who, n)| space.sealed(who, vec![field_op::create(ObjectId([n; 16]), TASK)]));
    let changes = [vec![genesis, add, envelope], tasks.to_vec()].concat();
    for change in &changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    // A duplicate counts once.
    view.receive(changes[4].bytes(), &mut checks).unwrap();
    changes
}

/// The counts of `who` by a full recount of the log.
fn recount(view: &View, space: SpaceId, who: u8) -> RelayCounts {
    let author = space::id(who);
    let own = |change: &Change| change.header().author == author;
    RelayCounts {
        unconfirmed: view
            .unconfirmed(space)
            .unwrap()
            .iter()
            .filter(|c| own(c))
            .count() as u64,
        refused: view
            .refused(space)
            .unwrap()
            .iter()
            .filter(|(c, _)| own(c))
            .count() as u64,
    }
}

/// The view's counts of A, B and C equal a full recount.
fn assert_current(view: &View, space: SpaceId, step: &str) {
    for who in [A, B, C] {
        assert_eq!(
            view.relay_counts(space, space::id(who)),
            recount(view, space, who),
            "member {who} after {step}"
        );
    }
}

#[test]
fn received_changes_count_as_unconfirmed_per_author() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);

    received(&mut view, &mut space);

    let a = view.relay_counts(space.space, space::id(A));
    assert_eq!(
        a,
        RelayCounts {
            unconfirmed: 5,
            refused: 0
        }
    );
    assert_current(&view, space.space, "receiving");
}

#[test]
fn the_counts_follow_confirmations_refusals_and_a_new_epoch() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);
    let changes = received(&mut view, &mut space);
    let at = space.space;

    view.confirm(at, changes[4].id(), 1).unwrap();
    assert_current(&view, at, "a confirmation");
    view.confirm(at, changes[4].id(), 1).unwrap();
    assert_current(&view, at, "the same confirmation again");
    view.refuse(at, changes[5].id(), "bad_signature").unwrap();
    assert_current(&view, at, "a refusal of an unconfirmed change");
    view.refuse(at, changes[5].id(), "membership_rule").unwrap();
    assert_current(&view, at, "a second refusal of the same change");
    view.refuse(at, changes[4].id(), "bad_signature").unwrap();
    assert_current(&view, at, "a refusal of a confirmed change");
    view.confirm(at, changes[5].id(), 2).unwrap();
    assert_current(&view, at, "a confirmation of a refused change");
    view.confirm(at, changes[0].id(), 3).unwrap();
    view.confirm(at, changes[6].id(), 4).unwrap();
    assert_current(&view, at, "more confirmations");

    view.forget_relay_positions().unwrap();

    assert_current(&view, at, "a new epoch");
    let b = view.relay_counts(at, space::id(B));
    assert_eq!(
        b,
        RelayCounts {
            unconfirmed: 1,
            refused: 1
        }
    );
}

#[test]
fn a_reopened_view_reads_the_counts_back() {
    let db = TempDb::new("relay-counts");
    let mut space = Space::new(A);
    {
        let mut view = View::open(&db.0, &lock()).unwrap();
        let changes = received(&mut view, &mut space);
        view.confirm(space.space, changes[0].id(), 1).unwrap();
        view.confirm(space.space, changes[4].id(), 2).unwrap();
        view.refuse(space.space, changes[5].id(), "membership_rule")
            .unwrap();
    }

    let view = View::open(&db.0, &lock()).unwrap();

    assert_current(&view, space.space, "a reopen");
    let a = view.relay_counts(space.space, space::id(A));
    assert_eq!(
        a,
        RelayCounts {
            unconfirmed: 3,
            refused: 1
        }
    );
}

#[test]
fn a_space_the_view_does_not_hold_counts_nothing() {
    let view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);

    assert_eq!(
        view.relay_counts(space.space, space::id(A)),
        RelayCounts::default()
    );
}
