//! Pulling and subscribing (RFC 0007, "Pull" and "Subscribe"): the
//! database joins the spaces `welcome` lists, pulls each from its cursor,
//! folds what it pulled and follows the space live.

use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role;

use super::support::*;
use crate::id::ObjectId;

/// The simulated admin who creates the space.
const ADMIN: Who = 0;
/// The member whose database syncs.
const ME: Who = 1;

const FIRST: CoreObjectId = CoreObjectId([0x11; 16]);
const SECOND: CoreObjectId = CoreObjectId([0x22; 16]);

/// A space of `ADMIN`'s on the relay, with `ME` as a writer that holds the
/// space key, and one task.
async fn shared(relay: &LocalRelay) -> (space::Space, Client) {
    let mut space = space::Space::new(ADMIN);
    let genesis = space.genesis.clone();
    let add = space.add(ADMIN, ME, Role::Writer, &[&genesis]);
    let own = space.envelope(ADMIN, ADMIN, &[&add]);
    let envelope = space.envelope(ADMIN, ME, &[&own]);
    let task = space.sealed(ADMIN, vec![field_op::create(FIRST, "tasks")]);
    let mut client = relay.client(ADMIN).await;
    push(&mut client, &[&genesis, &add, &own, &envelope, &task]).await;
    (space, client)
}

/// Whether the database holds the task `object` of `space`.
fn holds(db: &crate::Db, space: &space::Space, object: CoreObjectId) -> bool {
    let handle = handle(db, space.space);
    matches!(now(handle.get("tasks", ObjectId(object))), Ok(Some(_)))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_the_member_was_added_to_is_pulled_and_folded() {
    let relay = LocalRelay::start().await;
    let (space, _client) = shared(&relay).await;

    let db = open(ME, &relay.url()).await;

    eventually("the task", || holds(&db, &space, FIRST)).await;
    let view = &crate::db::lock(&db.inner).view;
    assert_eq!(
        view.relay_position(space.space, space.genesis.id()),
        Some(1)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_appended_later_arrives_through_the_subscription() {
    let relay = LocalRelay::start().await;
    let (mut space, mut client) = shared(&relay).await;
    let db = open(ME, &relay.url()).await;
    eventually("the first task", || holds(&db, &space, FIRST)).await;

    let later = space.sealed(ADMIN, vec![field_op::create(SECOND, "tasks")]);
    push(&mut client, &[&later]).await;

    eventually("the second task", || holds(&db, &space, SECOND)).await;
    let position = crate::db::lock(&db.inner)
        .view
        .relay_position(space.space, later.id());
    assert_eq!(position, Some(6));
}
