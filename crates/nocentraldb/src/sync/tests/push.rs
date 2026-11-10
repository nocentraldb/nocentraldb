//! Pushing (RFC 0007, "Push" and "Island mode", step 2): every change
//! without a relay position, in local order; the status counts this
//! member's changes the relay has not confirmed; a refusal is reported,
//! not thrown.

use super::support::*;
use crate::sync::Connection;
use crate::value::Value;

const ME: Who = 1;

#[tokio::test(flavor = "multi_thread")]
async fn a_new_space_and_its_writes_reach_the_relay_in_local_order() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;

    let space = db.spaces().create().await.unwrap();
    let first = space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("one"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let second = space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("two"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let status = synced(&db).await;

    let log = relay.log(space.id().0).await;
    assert_eq!(log.len(), 4, "the genesis, the own envelope and two writes");
    assert_eq!(&log[2..], [first.change.0, second.change.0]);
    assert_eq!(status.refused, 0);
    assert!(!status.epoch_changed);
    assert_eq!(status.error, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_status_counts_own_changes_the_relay_has_not_confirmed() {
    let relay = LocalRelay::start().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;

    let space = db.spaces().create().await.unwrap();
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("offline"))])?;
            Ok(())
        })
        .await
        .unwrap();

    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;
    assert_eq!(db.sync().current().unconfirmed, 3);
    relay.proxy.restore();
    synced(&db).await;
    assert_eq!(relay.log(space.id().0).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_database_without_a_relay_reports_none_and_counts_every_change() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK))
        .await
        .unwrap();

    db.spaces().create().await.unwrap();

    let status = db.sync().current();
    assert_eq!(status.connection, Connection::None);
    assert_eq!(status.unconfirmed, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_change_is_reported_not_thrown() {
    let relay = LocalRelay::closed().await;
    let db = open(ME, &relay.url()).await;

    let space = db.spaces().create().await.unwrap();
    let written = space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("kept"))])?;
            Ok(())
        })
        .await
        .unwrap();

    let mut status = db.sync().current();
    eventually("refused", || {
        status = db.sync().current();
        status.refused == 3
    })
    .await;
    assert_eq!(status.unconfirmed, 0);
    assert_eq!(status.connection, Connection::Connected);
    assert!(relay.log(space.id().0).await.is_empty());
    let kept = space.get("tasks", written.ids[0]).await.unwrap();
    assert!(kept.is_some(), "a refused change stays folded");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_while_connected_is_pushed_at_once() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    synced(&db).await;

    let written = space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("later"))])?;
            Ok(())
        })
        .await
        .unwrap();

    synced(&db).await;
    assert_eq!(relay.log(space.id().0).await[2], written.change.0);
}

#[tokio::test]
async fn only_a_plain_websocket_relay_is_accepted() {
    let options = crate::OpenOptions::new(LOCK).relay("wss://relay.example.com");
    let opened = crate::Db::open(options).await;
    let error = opened.expect_err("TLS is not built yet");
    assert_eq!(error.code(), crate::ErrorCode::InvalidValue);
}
