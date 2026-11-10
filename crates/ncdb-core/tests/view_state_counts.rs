//! The view reads each space's parked and rejected counts per reason
//! (RFC 0009, "Sync and status"), and a reopened view reads them back from
//! its log table.
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

use std::collections::BTreeMap;

use ncdb_core::change::Operation;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::{OperationParkReason, RejectReason, StateCounts};
use ncdb_core::membership::Role;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;
/// A member nobody added.
const C: u8 = 3;

/// B's view after B joins, with a change by C, whom nobody added, and a
/// change of A's with an operation on a collection the lock does not know.
fn received(view: &mut View, space: &mut Space) {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let stranger = space.sealed(C, vec![field_op::create(ObjectId([1; 16]), TASK)]);
    let later = Operation::new(ObjectId([2; 16]), "later".into(), None, 0, None);
    let mixed = space.sealed(A, vec![field_op::create(ObjectId([3; 16]), TASK), later]);
    for change in [genesis, add, envelope, stranger, mixed] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
}

fn expected() -> StateCounts {
    StateCounts {
        parked: BTreeMap::new(),
        parked_operations: BTreeMap::from([(OperationParkReason::UnknownCollection, 1)]),
        rejected: BTreeMap::from([(RejectReason::UnknownAuthor, 1)]),
    }
}

#[test]
fn the_view_reads_a_spaces_counts() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);

    received(&mut view, &mut space);

    assert_eq!(view.state_counts(space.space), expected());
}

#[test]
fn a_reopened_view_reads_the_counts_back() {
    let db = TempDb::new("state-counts");
    let mut space = Space::new(A);
    {
        let mut view = View::open(&db.0, &lock()).unwrap();
        received(&mut view, &mut space);
    }

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.state_counts(space.space), expected());
}

#[test]
fn a_space_the_view_does_not_hold_counts_nothing() {
    let view = View::open_in_memory(&lock()).unwrap();

    assert_eq!(
        view.state_counts(Space::new(A).space),
        StateCounts::default()
    );
}
