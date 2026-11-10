//! Pausing and resuming sync (RFC 0009, "Sync and status"): `pause()`
//! closes the connection, after which the SDK sends the relay nothing and
//! the status says `paused`; `resume()` connects again at once and pushes
//! what was written meanwhile. A pause lasts while the database is open.

use std::time::Duration;

use super::follow::next;
use super::support::*;
use crate::sync::Connection;
use crate::value::Value;

const ME: Who = 1;

#[tokio::test(flavor = "multi_thread")]
async fn a_paused_database_reports_paused_and_pushes_nothing() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    synced(&db).await;

    db.sync().pause().await;
    assert_eq!(db.sync().current().connection, Connection::Paused);
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("offline"))])?;
            Ok(())
        })
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let status = db.sync().current();
    assert_eq!(status.connection, Connection::Paused);
    assert_eq!(status.unconfirmed, 1);
    assert_eq!(
        relay.log(space.id().0).await.len(),
        2,
        "the write reached the relay"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_connects_and_pushes_what_was_written_while_paused() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    synced(&db).await;
    db.sync().pause().await;
    let written = space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("later"))])?;
            Ok(())
        })
        .await
        .unwrap();

    db.sync().resume().await;

    synced(&db).await;
    assert_eq!(relay.log(space.id().0).await[2], written.change.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pause_while_offline_holds_until_resume() {
    let relay = LocalRelay::start().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;

    db.sync().pause().await;
    relay.proxy.restore();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert_eq!(db.sync().current().connection, Connection::Paused);

    db.sync().resume().await;
    synced(&db).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stream_follows_a_pause() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    synced(&db).await;
    let mut statuses = db.sync().subscribe();
    next(&mut statuses).await;

    db.sync().pause().await;

    assert_eq!(next(&mut statuses).await.connection, Connection::Paused);
}

#[tokio::test]
async fn pause_and_resume_without_a_relay_change_nothing() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();

    db.sync().pause().await;
    assert_eq!(db.sync().current().connection, Connection::None);
    db.sync().resume().await;
    assert_eq!(db.sync().current().connection, Connection::None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_resume_right_after_a_pause_drops_the_connection_and_connects_again() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    synced(&db).await;

    for _ in 0..20 {
        db.sync().pause().await;
        db.sync().resume().await;
        synced(&db).await;
    }
}
