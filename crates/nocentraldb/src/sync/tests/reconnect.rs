//! Reconnecting (RFC 0007, "Island mode" and "Restores and the relay
//! epoch"): on reconnect the database pulls what it missed and pushes what
//! the relay has not confirmed; a new relay epoch makes it pull from 0 and
//! push again, without folding anything twice.

use ncdb_core::field_op;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::membership::Role;

use super::support::*;
use crate::id::ObjectId;
use crate::sync::Connection;
use crate::value::Value;

const ADMIN: Who = 0;
const ME: Who = 1;

const FIRST: CoreObjectId = CoreObjectId([0x11; 16]);
const MISSED: CoreObjectId = CoreObjectId([0x33; 16]);

/// `ADMIN`'s space on the relay with `ME` as a writer, and a task.
async fn shared(relay: &LocalRelay) -> (space::Space, Client) {
    let mut space = space::Space::new(ADMIN);
    let genesis = space.genesis.clone();
    let add = space.add(ADMIN, ME, Role::Writer, &[&genesis]);
    let envelope = space.envelope(ADMIN, ME, &[&add]);
    let task = space.sealed(ADMIN, vec![field_op::create(FIRST, "tasks")]);
    let mut client = relay.client(ADMIN).await;
    push(&mut client, &[&genesis, &add, &envelope, &task]).await;
    (space, client)
}

fn holds(db: &crate::Db, space: ncdb_core::ids::SpaceId, object: CoreObjectId) -> bool {
    matches!(
        now(handle(db, space).get("tasks", ObjectId(object))),
        Ok(Some(_))
    )
}

async fn offline(db: &crate::Db) {
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reconnect_pushes_what_was_written_offline_and_pulls_what_was_missed() {
    let relay = LocalRelay::start().await;
    let (mut space, mut client) = shared(&relay).await;
    let db = open(ME, &relay.url()).await;
    let own = db.spaces().create().await.unwrap();
    eventually("the shared task", || holds(&db, space.space, FIRST)).await;
    synced(&db).await;

    relay.proxy.cut();
    offline(&db).await;
    let written = own
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("offline"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let missed = space.sealed(ADMIN, vec![field_op::create(MISSED, "tasks")]);
    push(&mut client, &[&missed]).await;
    assert_eq!(db.sync().current().unconfirmed, 1);
    relay.proxy.restore();

    eventually("the missed task", || holds(&db, space.space, MISSED)).await;
    synced(&db).await;
    assert_eq!(relay.log(own.id().0).await[2], written.change.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restore_makes_the_member_pull_from_zero_and_push_again_without_folding_twice() {
    let mut relay = LocalRelay::start().await;
    let (space, _client) = shared(&relay).await;
    let db = open(ME, &relay.url()).await;
    let own = db.spaces().create().await.unwrap();
    eventually("the shared task", || holds(&db, space.space, FIRST)).await;
    synced(&db).await;
    let backup = relay.backup();
    let lost = own
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("lost in the restore"))])?;
            Ok(())
        })
        .await
        .unwrap();
    synced(&db).await;

    relay.proxy.cut();
    offline(&db).await;
    relay.restore(backup).await;
    assert_eq!(relay.log(own.id().0).await.len(), 2, "the restore lost it");
    relay.proxy.restore();

    eventually("pushed again", || now(relay.log(own.id().0)).len() == 3).await;
    synced(&db).await;
    eventually("every space synced in the new epoch", || {
        !db.sync().current().epoch_changed
    })
    .await;
    assert_eq!(relay.log(own.id().0).await[2], lost.change.0);
    let inner = crate::db::lock(&db.inner);
    assert_eq!(inner.sync_folds(), 4, "the shared space's changes, once");
    let genesis = inner.view.relay_position(space.space, space.genesis.id());
    assert_eq!(genesis, Some(1), "pulled again from 0");
}
