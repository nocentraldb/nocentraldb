//! The view reads the newest model version a space's parked data needs,
//! against the `model.id` of its own lock (RFC 0002, "What the model
//! version is for"), also after a reopen, since it reads it from the frames.
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

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Operation, Payload, WriterModel};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::membership::Role;
use ncdb_core::seal;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

/// A change of A under `sequence`, written by the lock's model at
/// `version`, with an operation on a collection the lock does not know.
fn newer(space: &Space, sequence: u64, version: u64) -> Change {
    let later = Operation::new(ObjectId([9; 16]), "later".into(), None, 0, None);
    let operations = vec![field_op::create(ObjectId([1; 16]), TASK), later];
    let mut payload = Payload::new(Hlc(sequence << 16), BTreeSet::new(), operations);
    payload.set_writer_model(WriterModel {
        id: lock().model.id.0,
        version,
    });
    seal::seal(
        &space::key(A),
        &space::space_key(),
        space.space,
        sequence,
        space.first_generation,
        &payload,
    )
}

/// B's view after B joins and A writes two changes of newer versions.
fn received(view: &mut View, space: &mut Space) -> SealChecks {
    let mut checks = SealChecks::new(space.space, space::key(B));
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let next = space.sequence(A) + 1;
    let changes = [
        genesis,
        add,
        envelope,
        newer(space, next, 4),
        newer(space, next + 1, 3),
    ];
    for change in &changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    checks
}

#[test]
fn the_view_reads_the_newest_model_its_parked_data_needs() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let mut space = Space::new(A);

    let checks = received(&mut view, &mut space);

    assert_eq!(view.newest_model(space.space, &checks), Some(4));
}

#[test]
fn a_reopened_view_reads_it_again_from_the_frames() {
    let db = TempDb::new("newest-model");
    let mut space = Space::new(A);
    let checks = {
        let mut view = View::open(&db.0, &lock()).unwrap();
        received(&mut view, &mut space)
    };

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.newest_model(space.space, &checks), Some(4));
}
