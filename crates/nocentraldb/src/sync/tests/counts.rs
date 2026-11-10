//! The counts of the sync status (NOC-147): `unconfirmed` and `refused`
//! are kept current as the member writes, the relay confirms and refuses,
//! and the database reopens, and always equal a full recount of the
//! member's own changes in the log, with and without a relay.

use super::support::*;
use crate::db::{Db, OpenOptions};
use crate::keys::{Keys, Secrets};
use crate::space::Space;
use crate::sync::Connection;
use crate::value::Value;

const ME: Who = 1;

/// This member's changes the relay has not confirmed, and those it
/// refused, by decoding every change of every space's log.
fn recount(db: &Db) -> (u64, u64) {
    let inner = db.lock();
    let me = inner.member.key().id();
    let (mut unconfirmed, mut refused) = (0, 0);
    for space in inner.view.spaces() {
        let changes = inner.view.unconfirmed(space).unwrap();
        unconfirmed += changes.iter().filter(|c| c.header().author == me).count() as u64;
        let changes = inner.view.refused(space).unwrap();
        refused += changes
            .iter()
            .filter(|(c, _)| c.header().author == me)
            .count() as u64;
    }
    (unconfirmed, refused)
}

/// The counts the sync status reports, which must equal the recount.
fn counted(db: &Db, step: &str) -> (u64, u64) {
    let status = db.sync().current();
    let counts = (status.unconfirmed, status.refused);
    assert_eq!(counts, recount(db), "after {step}");
    counts
}

async fn write(space: &Space, title: &str) {
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from(title))])?;
            Ok(())
        })
        .await
        .unwrap();
}

/// The database of `ME` at `dir`, with `ME`'s keys in a key file, syncing
/// with the relay at `url` when one is given.
async fn open_at(dir: &TempDir, url: Option<&str>) -> Db {
    let key_file = dir.0.join("member.key");
    if !key_file.exists() {
        let keys = Keys {
            member: Secrets::new([ME; 32], [ME ^ 0x80; 32]),
            account: account(),
        };
        std::fs::write(&key_file, keys.encode()).unwrap();
    }
    let options = OpenOptions::new(LOCK)
        .path(dir.0.join("data"))
        .key_file(key_file);
    let options = match url {
        Some(url) => options.relay(url),
        None => options,
    };
    Db::open(options).await.expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_relay_every_write_counts_and_a_reopen_reads_them_back() {
    let dir = TempDir::new();
    let db = open_at(&dir, None).await;
    assert_eq!(counted(&db, "opening"), (0, 0));

    let space = db.spaces().create().await.unwrap();
    assert_eq!(counted(&db, "creating a space"), (2, 0));
    write(&space, "one").await;
    write(&space, "two").await;
    assert_eq!(counted(&db, "two writes"), (4, 0));
    drop((space, db));

    let db = open_at(&dir, None).await;
    assert_eq!(counted(&db, "a reopen"), (4, 0));
    let space = db.spaces().get(only_space(&db)).await.unwrap();
    write(&space, "three").await;
    assert_eq!(counted(&db, "a write after the reopen"), (5, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn confirmations_lower_the_count_and_a_write_offline_raises_it() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    write(&space, "online").await;
    synced(&db).await;
    assert_eq!(counted(&db, "the relay confirmed every change"), (0, 0));

    relay.proxy.cut();
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;
    write(&space, "offline").await;
    assert_eq!(counted(&db, "a write offline"), (1, 0));

    relay.proxy.restore();
    synced(&db).await;
    assert_eq!(counted(&db, "pushing it"), (0, 0));
}

#[tokio::test(flavor = "multi_thread")]
async fn refusals_move_changes_from_unconfirmed_to_refused() {
    let relay = LocalRelay::closed().await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    write(&space, "refused").await;

    eventually("refused", || db.sync().current().refused == 3).await;

    assert_eq!(counted(&db, "the relay refused every change"), (0, 3));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reopen_with_a_relay_counts_what_it_confirms_again() {
    let relay = LocalRelay::start().await;
    let dir = TempDir::new();
    let db = open_at(&dir, Some(&relay.url())).await;
    let space = db.spaces().create().await.unwrap();
    write(&space, "confirmed").await;
    synced(&db).await;
    relay.proxy.cut();
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;
    write(&space, "offline").await;
    assert_eq!(counted(&db, "a write offline"), (1, 0));
    drop((space, db));

    let db = open_at(&dir, Some(&relay.url())).await;
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;
    assert_eq!(counted(&db, "a reopen while offline"), (1, 0));

    relay.proxy.restore();
    synced(&db).await;
    assert_eq!(counted(&db, "syncing after the reopen"), (0, 0));
}

/// The id of the one space the database holds.
fn only_space(db: &Db) -> crate::SpaceId {
    let inner = db.lock();
    let space = inner.view.spaces().next().expect("one space");
    crate::SpaceId(space)
}
