//! The sync part of a space's status (RFC 0009, "Sync and status"):
//! whether the first pull reached the relay's head, how many relay
//! positions the member heard of but has not folded, and the stream that
//! follows the status.

use std::time::Duration;

use futures_core::Stream;

use super::support::*;
use crate::status::SpaceStatus;
use crate::value::Value;

const ME: Who = 1;

/// The next status `stream` yields, within a few seconds.
async fn next(stream: &mut (impl Stream<Item = SpaceStatus> + Unpin)) -> SpaceStatus {
    let item = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx));
    tokio::time::timeout(Duration::from_secs(20), item)
        .await
        .expect("a status in time")
        .expect("the stream does not end")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_pulled_to_the_relays_head_completed_its_initial_sync() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    assert!(!space.status().current().initial_sync_complete);

    synced(&db).await;
    eventually("the initial sync", || {
        space.status().current().initial_sync_complete
    })
    .await;

    assert_eq!(space.status().current().behind, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn behind_counts_the_positions_heard_of_but_not_folded() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    synced(&db).await;
    eventually("the initial sync", || {
        space.status().current().initial_sync_complete
    })
    .await;
    relay.proxy.cut();
    eventually("offline", || {
        db.sync().current().connection == crate::sync::Connection::Offline
    })
    .await;

    {
        let mut inner = crate::db::lock(&db.inner);
        let sync = inner.sync.as_mut().unwrap();
        // A space the relay did not hold before its first push may have
        // no cursor yet: its cursor is 0.
        let cursor = sync.cursors.get(&space.id().0).copied().unwrap_or(0);
        sync.heads.insert(space.id().0, cursor + 3);
    }

    assert_eq!(space.status().current().behind, 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_stream_yields_the_status_at_once_and_follows_refusals() {
    let relay = LocalRelay::closed().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("refused"))])?;
            Ok(())
        })
        .await
        .unwrap();
    let mut statuses = space.status().subscribe();
    assert_eq!(next(&mut statuses).await, space.status().current());

    relay.proxy.restore();

    let mut status = next(&mut statuses).await;
    while status.refused.len() < 3 {
        status = next(&mut statuses).await;
    }
    for refused in &status.refused {
        let mapped = crate::sync::wire::mapped_code(&refused.code);
        assert_eq!(mapped, refused.code, "not a code RFC 0007 lists");
    }
}
