//! The status of one space (RFC 0009, "Sync and status": `space.status`),
//! read from what the member's copy folded: the member's role, the gaps,
//! the parked and rejected changes per reason, the relay's refusals of the
//! member's own changes by code, the lost grants, whether the current key
//! generation is exposed, and whether the member waits for its key.
//!
//! The database syncs with no relay here: the changes of a simulated
//! space, whose admins are simulated members, are folded into it through
//! the crate's internals, as syncing folds what it pulls.

use std::collections::BTreeMap;

use ncdb_core::change::{Change, Operation};
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role as CoreRole;

use super::rotation::model_id;
use super::support::*;
use crate::db::{Db, OpenOptions, SpaceState};
use crate::id::{ChangeId, SpaceId};
use crate::members::Role;
use crate::status::{ParkReason, Refused, RejectReason, SpaceStatus};

/// The senior admin, who creates the space.
const X: Who = 0;
/// The member whose database this is, a writer.
const ME: Who = 1;
/// A second admin.
const Y: Who = 3;
/// A writer the admins add, then disagree about.
const Z: Who = 4;
/// A member nobody added.
const STRANGER: Who = 5;

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
/// into `db`. Returns the space and the last change every admin saw.
fn joined(db: &Db) -> (space::Space, Change) {
    let mut space = space::Space::with_model(X, [5; 16], model_id());
    let genesis = space.genesis.clone();
    let own = space.envelope(X, X, &[&genesis]);
    let add = space.add(X, ME, CoreRole::Writer, &[&own]);
    let last = space.envelope(X, ME, &[&add]);
    for change in [&genesis, &own, &add, &last] {
        fold(db, change);
    }
    (space, last)
}

fn status(db: &Db, space: &space::Space) -> SpaceStatus {
    handle(db, space.space).status().current()
}

#[tokio::test]
async fn a_space_the_member_created_reports_its_role_and_nothing_else() {
    let db = mine().await;

    let space = db.spaces().create().await.unwrap();
    let status = space.status().current();

    assert_eq!(status.role, Some(Role::Admin));
    assert!(!status.initial_sync_complete, "no relay to sync with");
    assert_eq!(status.behind, 0);
    assert!(status.gaps.is_empty());
    assert!(status.parked.is_empty());
    assert!(status.rejected.is_empty());
    assert!(status.refused.is_empty());
    assert!(status.lost_grants.is_empty());
    assert!(!status.exposed);
    assert!(!status.waiting_for_key);
}

#[tokio::test]
async fn parked_operations_and_rejected_changes_count_per_reason() {
    let db = mine().await;
    let (mut space, _) = joined(&db);
    let later = Operation::new(CoreObjectId([2; 16]), "later".into(), None, 0, None);
    let mixed = space.sealed(
        X,
        vec![field_op::create(CoreObjectId([1; 16]), "tasks"), later],
    );
    let stranger = space.sealed(STRANGER, vec![]);
    fold(&db, &mixed);
    fold(&db, &stranger);

    let status = status(&db, &space);

    assert_eq!(status.role, Some(Role::Writer));
    assert_eq!(
        status.parked,
        BTreeMap::from([(ParkReason::UnknownCollection, 1)])
    );
    assert_eq!(
        status.rejected,
        BTreeMap::from([(RejectReason::UnknownAuthor, 1)])
    );
}

#[tokio::test]
async fn a_control_dependency_the_copy_lacks_is_a_gap() {
    let db = mine().await;
    let (mut space, last) = joined(&db);
    let missing = space.add(X, Z, CoreRole::Writer, &[&last]);
    let after = space.envelope(X, Z, &[&missing]);
    fold(&db, &after);

    let status = status(&db, &space);

    assert_eq!(status.gaps, [ChangeId(missing.id())]);
}

#[tokio::test]
async fn a_grant_that_did_not_see_a_concurrent_removal_is_lost() {
    let db = mine().await;
    let (mut space, last) = joined(&db);
    let add_y = space.add(X, Y, CoreRole::Admin, &[&last]);
    let add_z = space.add(X, Z, CoreRole::Writer, &[&add_y]);
    let to_z = space.envelope(X, Z, &[&add_z]);
    // X removes Z while Y, not having seen it, raises Z to admin.
    let removal = space.remove(X, Z, 0, &[&to_z]);
    let raise = space.set_role(Y, Z, CoreRole::Admin, 0, &[&to_z]);
    for change in [&add_y, &add_z, &to_z, &removal, &raise] {
        fold(&db, change);
    }

    let status = status(&db, &space);

    assert_eq!(status.lost_grants, [ChangeId(raise.id())]);
    assert!(
        status.exposed,
        "Z holds the current key and is no longer a member"
    );
}

#[tokio::test]
async fn a_rotation_whose_envelope_the_member_lacks_makes_it_wait_for_the_key() {
    let db = mine().await;
    let (mut space, last) = joined(&db);
    let first = space.first_generation;
    let (rotation, _) = space.rotate(X, &[first], &[&last]);
    fold(&db, &rotation);

    let status = status(&db, &space);

    assert!(status.waiting_for_key);
    assert_eq!(status.role, Some(Role::Writer));
}

#[tokio::test]
async fn a_removed_member_reports_no_role() {
    let db = mine().await;
    let (mut space, last) = joined(&db);
    let removal = space.remove(X, ME, 0, &[&last]);
    fold(&db, &removal);

    assert_eq!(status(&db, &space).role, None);
}

#[tokio::test]
async fn the_relays_refusals_of_own_changes_show_with_their_mapped_codes() {
    let db = mine().await;
    let space = db.spaces().create().await.unwrap();
    let written = space
        .write(|tx| {
            tx.create("tasks", [("title", crate::Value::from("refused"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let genesis = {
        let inner = crate::db::lock(&db.inner);
        inner.view.unconfirmed(space.id().0).unwrap()[0].id()
    };
    {
        // A log written before NOC-188 may hold the relay's own text.
        let mut inner = crate::db::lock(&db.inner);
        inner
            .view
            .refuse(space.id().0, genesis, "the relay's own text")
            .unwrap();
        inner
            .view
            .refuse(space.id().0, written.change.0, "author_not_member")
            .unwrap();
    }

    let refused = space.status().current().refused;

    assert_eq!(
        refused,
        [
            Refused {
                change: ChangeId(genesis),
                code: "unknown_error".into()
            },
            Refused {
                change: written.change,
                code: "author_not_member".into()
            },
        ]
    );
}

#[tokio::test]
async fn a_space_without_a_relay_is_neither_synced_nor_behind() {
    let db = mine().await;
    let (space, _) = joined(&db);

    let status = status(&db, &space);

    assert!(!status.initial_sync_complete);
    assert_eq!(status.behind, 0);
    let _ = SpaceId(space.space);
}
