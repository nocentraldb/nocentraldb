//! A database at a path, reopened (NOC-108): the relay positions in its
//! log were confirmed in a relay epoch the reopened database no longer
//! knows, so a relay restored while it was closed must not keep a change
//! it lost marked as confirmed (RFC 0007, "Restores and the relay epoch").

use super::support::*;
use crate::db::OpenOptions;
use crate::keys::{Keys, Secrets};
use crate::value::Value;

const ME: Who = 1;

/// The database of `ME`, of the simulated account, at `dir`, syncing with
/// the relay at `url`. Its key file holds `ME`'s keys, so the relay lets
/// it create spaces.
pub(super) async fn open_at(dir: &TempDir, url: &str) -> crate::Db {
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
        .key_file(key_file)
        .relay(url);
    crate::Db::open(options).await.expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_database_reopened_after_a_restore_pushes_again_what_the_relay_lost() {
    let mut relay = LocalRelay::start().await;
    let dir = TempDir::new();
    let db = open_at(&dir, &relay.url()).await;
    let own = db.spaces().create().await.unwrap();
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
    let space = own.id().0;
    drop((own, db));

    relay.proxy.cut();
    relay.restore(backup).await;
    assert_eq!(relay.log(space).await.len(), 2, "the restore lost it");
    relay.proxy.restore();
    let db = open_at(&dir, &relay.url()).await;

    eventually("pushed again", || now(relay.log(space)).len() == 3).await;
    synced(&db).await;
    assert_eq!(relay.log(space).await[2], lost.change.0);
    assert!(!db.sync().current().epoch_changed);
}

/// The relay epoch and the cursor of `space`, as the database keeps them.
fn epoch_and_cursor(db: &crate::Db, space: ncdb_core::ids::SpaceId) -> (Option<[u8; 16]>, u64) {
    let inner = crate::db::lock(&db.inner);
    let sync = inner.sync.as_ref().expect("a database that syncs");
    (sync.epoch, sync.cursors.get(&space).copied().unwrap_or(0))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reopened_database_keeps_the_relay_epoch_and_starts_at_its_cursor() {
    let relay = LocalRelay::start().await;
    let dir = TempDir::new();
    let db = open_at(&dir, &relay.url()).await;
    let own = db.spaces().create().await.unwrap();
    let written = own
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("kept"))])?;
            Ok(())
        })
        .await
        .unwrap();
    synced(&db).await;
    let space = own.id().0;
    // The cursor reaches the relay's head once the database pulled its own
    // changes back.
    let head = relay.log(space).await.len() as u64;
    eventually("pulled up to the head", || {
        epoch_and_cursor(&db, space).1 == head
    })
    .await;
    let (epoch, cursor) = epoch_and_cursor(&db, space);
    assert!(epoch.is_some());
    drop((own, db));

    // Reopened while the relay cannot be reached: the epoch and the cursor
    // come back from the database, before any `welcome`.
    relay.proxy.cut();
    let db = open_at(&dir, &relay.url()).await;
    assert_eq!(epoch_and_cursor(&db, space), (epoch, cursor));
    assert!(db.sync().is_confirmed(written.change));

    // The same epoch in `welcome` keeps the relay positions.
    relay.proxy.restore();
    synced(&db).await;
    assert!(db.sync().is_confirmed(written.change));
    assert!(!db.sync().current().epoch_changed);
    assert_eq!(epoch_and_cursor(&db, space), (epoch, cursor));
}
