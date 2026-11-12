//! Following the sync report (RFC 0009, "Sync and status"):
//! `db.sync().subscribe()` yields the status at once, and again whenever
//! it changes, the connection, the counts and the error alike, through a
//! reconnect too; a status that did not change is not yielded twice.

use std::time::Duration;

use futures_core::Stream;

use super::support::*;
use crate::sync::{Connection, SyncStatus};
use crate::value::Value;

const ME: Who = 1;

/// The next status `stream` yields, within a few seconds.
pub async fn next(stream: &mut (impl Stream<Item = SyncStatus> + Unpin)) -> SyncStatus {
    let item = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx));
    tokio::time::timeout(Duration::from_secs(20), item)
        .await
        .expect("a status in time")
        .expect("the stream does not end")
}

/// The statuses `stream` yields until one satisfies `done`.
async fn until(
    stream: &mut (impl Stream<Item = SyncStatus> + Unpin),
    done: impl Fn(&SyncStatus) -> bool,
) -> SyncStatus {
    loop {
        let status = next(stream).await;
        if done(&status) {
            return status;
        }
    }
}

#[tokio::test]
async fn the_stream_yields_the_status_at_once() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();

    let mut statuses = db.sync().subscribe();

    assert_eq!(next(&mut statuses).await, db.sync().current());
}

#[tokio::test]
async fn a_write_without_a_relay_yields_the_new_count() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let mut statuses = db.sync().subscribe();
    assert_eq!(next(&mut statuses).await.unconfirmed, 0);

    db.spaces().create().await.unwrap();

    let status = next(&mut statuses).await;
    assert_eq!(status.connection, Connection::None);
    assert_eq!(status.unconfirmed, 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stream_follows_the_connection_and_the_counts() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let mut statuses = db.sync().subscribe();
    until(&mut statuses, |s| s.connection == Connection::Connected).await;

    let space = db.spaces().create().await.unwrap();
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("followed"))])?;
            Ok(())
        })
        .await
        .unwrap();
    until(&mut statuses, |s| s.unconfirmed > 0).await;
    until(&mut statuses, |s| s.unconfirmed == 0).await;

    relay.proxy.cut();
    until(&mut statuses, |s| s.connection == Connection::Offline).await;
}

/// A subscriber sees the connection come back after the relay was out of
/// reach, and the change written meanwhile confirmed on the reconnect
/// (NOC-240).
#[tokio::test(flavor = "multi_thread")]
async fn the_stream_follows_a_reconnect_and_the_push_it_confirms() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let mut statuses = db.sync().subscribe();
    until(&mut statuses, |s| {
        s.connection == Connection::Connected && s.unconfirmed == 0
    })
    .await;

    relay.proxy.cut();
    until(&mut statuses, |s| s.connection == Connection::Offline).await;
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("written offline"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let waiting = until(&mut statuses, |s| s.unconfirmed == 1).await;
    assert_ne!(waiting.connection, Connection::Connected);

    relay.proxy.restore();
    // The stream yields the whole status, not each step: on a loaded
    // machine the reconnect and the confirmation can land in one status, so
    // the test waits for both at once (NOC-259).
    until(&mut statuses, |s| {
        s.connection == Connection::Connected && s.unconfirmed == 0
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_status_that_did_not_change_is_not_yielded_again() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    synced(&db).await;
    let mut statuses = db.sync().subscribe();
    let first = next(&mut statuses).await;

    // A ping round-trip and idle time change nothing in the status.
    tokio::time::sleep(Duration::from_secs(1)).await;

    let more = std::future::poll_fn(|cx| std::pin::Pin::new(&mut statuses).poll_next(cx));
    let more = tokio::time::timeout(Duration::from_millis(200), more).await;
    assert!(more.is_err(), "yielded again: {more:?} after {first:?}");
}
