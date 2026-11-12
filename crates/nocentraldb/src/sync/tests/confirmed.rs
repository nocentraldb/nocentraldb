//! Confirmation of one write (RFC 0009, "Sync and status"):
//! `is_confirmed(change)` says whether the relay recorded a position for
//! the change, and `confirmed(change)` resolves when it does. It may never
//! resolve: the relay can refuse the change, or never be reached.

use std::time::Duration;

use super::support::*;
use crate::value::Value;

const ME: Who = 1;

async fn write(space: &crate::Space) -> crate::Written {
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("confirmed?"))])?;
            Ok(())
        })
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_is_confirmed_once_the_relay_holds_it() {
    let relay = LocalRelay::start().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let written = write(&space).await;
    assert!(!db.sync().is_confirmed(written.change));

    relay.proxy.restore();
    synced(&db).await;

    assert!(db.sync().is_confirmed(written.change));
}

#[tokio::test(flavor = "multi_thread")]
async fn confirmed_resolves_when_the_relay_records_a_position() {
    let relay = LocalRelay::start().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let written = write(&space).await;
    let early = tokio::time::timeout(
        Duration::from_millis(300),
        db.sync().confirmed(written.change),
    )
    .await;
    assert!(early.is_err(), "confirmed while the relay was cut off");

    relay.proxy.restore();
    tokio::time::timeout(Duration::from_secs(20), db.sync().confirmed(written.change))
        .await
        .expect("confirmed in time");
    assert!(db.sync().is_confirmed(written.change));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_change_is_not_confirmed() {
    let relay = LocalRelay::closed().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let written = write(&space).await;
    // The first space brings the account space: its genesis, the
    // creator's envelope and the device's records (RFC 0010).
    eventually("refused", || db.sync().current().refused == 6).await;

    assert!(!db.sync().is_confirmed(written.change));
    let waited = tokio::time::timeout(
        Duration::from_millis(300),
        db.sync().confirmed(written.change),
    )
    .await;
    assert!(waited.is_err(), "a refused change resolved as confirmed");
}

#[tokio::test]
async fn without_a_relay_no_change_is_confirmed() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let space = db.spaces().create().await.unwrap();
    let written = write(&space).await;

    assert!(!db.sync().is_confirmed(written.change));
}

#[tokio::test]
async fn a_change_the_database_does_not_hold_is_not_confirmed() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();

    let unknown = "00".repeat(32).parse().unwrap();

    assert!(!db.sync().is_confirmed(unknown));
}
