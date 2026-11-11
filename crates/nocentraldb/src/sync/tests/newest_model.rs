//! What a space's parked data needs of the app's model (RFC 0009, "Sync
//! and status": `parked` and `newest_model`; RFC 0002, "The writer's
//! model"): a change whose payload key 64 names another model is parked
//! as `ParkReason::AnotherModel`, counted apart, and `newest_model` gives
//! the highest version of the app's own model that the parked changes and
//! operations need. A version above `u32::MAX` reads as `u32::MAX` (pm,
//! 2026-11-11, NOC-197).
//!
//! The changes of a simulated space are folded into the database through
//! the crate's internals, as syncing folds what it pulls.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Operation, Payload, WriterModel};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role as CoreRole;
use ncdb_core::seal;

use super::rotation::model_id;
use super::support::*;
use crate::db::{Db, OpenOptions, SpaceState};
use crate::status::{ParkReason, SpaceStatus};

/// The admin, who creates the space and writes the changes.
const X: Who = 0;
/// The member whose database this is, a writer.
const ME: Who = 1;

/// `ME`'s database, in memory, without a relay.
async fn mine() -> Db {
    let options = OpenOptions::new(LOCK).in_memory();
    Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

/// Folds `change` into `db`, as syncing folds a change it pulls.
fn fold(db: &Db, change: &Change) {
    let mut inner = crate::db::lock(&db.inner);
    let space = change.header().space;
    let me = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces
        .entry(space)
        .or_insert_with(|| SpaceState::new(space, me));
    view.receive(change.bytes(), &mut state.checks)
        .expect("the change is appended");
}

/// A space `X` created, with `ME` a writer holding the first key, folded
/// into `db`.
fn joined(db: &Db) -> space::Space {
    let mut space = space::Space::with_model(X, [5; 16], model_id());
    let genesis = space.genesis.clone();
    let own = space.envelope(X, X, &[&genesis]);
    let add = space.add(X, ME, CoreRole::Writer, &[&own]);
    let last = space.envelope(X, ME, &[&add]);
    for change in [&genesis, &own, &add, &last] {
        fold(db, change);
    }
    space
}

/// A change of `X` at its next sequence, written by model `id` at
/// `version`: a task, and an operation on a collection the lock does not
/// know.
fn written_by(space: &mut space::Space, id: [u8; 16], version: u64) -> Change {
    let sequence = space.sequence(X) + 1;
    space.skip(X, 1);
    let later = Operation::new(CoreObjectId([2; 16]), "later".into(), None, 0, None);
    let operations = vec![field_op::create(CoreObjectId([1; 16]), "tasks"), later];
    let mut payload = Payload::new(Hlc(sequence << 16), BTreeSet::new(), operations);
    payload.set_writer_model(WriterModel { id, version });
    seal::seal(
        &space::key(X),
        &space::space_key(),
        space.space,
        sequence,
        space.first_generation,
        &payload,
    )
}

fn status(db: &Db, space: &space::Space) -> SpaceStatus {
    handle(db, space.space).status().current()
}

#[tokio::test]
async fn a_space_without_parked_data_needs_no_newer_model() {
    let db = mine().await;
    let space = joined(&db);

    assert_eq!(status(&db, &space).newest_model, None);
}

#[tokio::test]
async fn newest_model_is_the_highest_version_of_the_apps_model_its_parked_data_needs() {
    let db = mine().await;
    let mut space = joined(&db);
    let four = written_by(&mut space, model_id(), 4);
    let three = written_by(&mut space, model_id(), 3);
    fold(&db, &four);
    fold(&db, &three);

    let status = status(&db, &space);

    assert_eq!(status.newest_model, Some(4));
    assert_eq!(
        status.parked,
        BTreeMap::from([(ParkReason::UnknownCollection, 2)])
    );
}

#[tokio::test]
async fn a_change_of_another_model_is_parked_apart_and_needs_no_version_of_this_one() {
    let db = mine().await;
    let mut space = joined(&db);
    let mut other = model_id();
    other[0] ^= 0xff;
    let foreign = written_by(&mut space, other, 9);
    fold(&db, &foreign);

    let status = status(&db, &space);

    assert_eq!(
        status.parked,
        BTreeMap::from([(ParkReason::AnotherModel, 1)])
    );
    assert_eq!(status.newest_model, None);
}

#[tokio::test]
async fn a_version_above_u32_reads_as_the_largest_u32() {
    let db = mine().await;
    let mut space = joined(&db);
    let huge = written_by(&mut space, model_id(), u64::from(u32::MAX) + 1);
    fold(&db, &huge);

    assert_eq!(status(&db, &space).newest_model, Some(u32::MAX));
}
