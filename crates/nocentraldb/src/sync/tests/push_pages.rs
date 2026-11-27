//! The push reads the unconfirmed changes of a space a push at a time
//! (NOC-519): a burst of writes that outpaces the push leaves many
//! changes unconfirmed, and decoding them all at once took the writer's
//! peak memory to 139 MB at 300,000 records. Every change still reaches
//! the relay, in local order, as before.

use super::support::*;
use crate::sync::{Connection, pages, wire};
use crate::value::Value;

const ME: Who = 1;

/// More changes than two pushes carry.
const BURST: usize = 2 * wire::MAX_PUSH + 6;

async fn burst(space: &crate::space::Space) -> Vec<ncdb_core::ids::ChangeId> {
    let mut ids = Vec::new();
    for i in 0..BURST {
        let written = space
            .write(|tx| {
                tx.create("tasks", [("title", Value::from(format!("task {i}")))])?;
                Ok(())
            })
            .await
            .unwrap();
        ids.push(written.change.0);
    }
    ids
}

#[tokio::test(flavor = "multi_thread")]
async fn the_push_reads_no_more_changes_than_one_push_carries() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let space = db.spaces().create().await.unwrap();
    burst(&space).await;

    let mut inner = crate::db::lock(&db.inner);
    let page = pages::next(&mut inner, space.id().0, 0, u64::MAX).unwrap();
    let read: Vec<_> = page.pushes.iter().flat_map(|(ids, _)| ids).collect();
    assert!(
        read.len() <= wire::MAX_PUSH,
        "the push read {} unconfirmed changes at once",
        read.len()
    );
}

/// Changes of about 3 KB each fill a push by its bytes, not by its count,
/// so a read of [`wire::MAX_PUSH`] of them ends in a push that the next
/// read's changes could still fill. The pushes are those of reading all
/// unconfirmed changes at once.
#[tokio::test(flavor = "multi_thread")]
async fn the_pushes_are_those_of_reading_every_change_at_once() {
    let db = crate::Db::open(crate::OpenOptions::new(LOCK).in_memory())
        .await
        .unwrap();
    let space = db.spaces().create().await.unwrap();
    for i in 0..BURST {
        space
            .write(|tx| {
                let title = format!("task {i} {}", "x".repeat(3000));
                tx.create("tasks", [("title", Value::from(title))])?;
                Ok(())
            })
            .await
            .unwrap();
    }
    let mut inner = crate::db::lock(&db.inner);
    let space = space.id().0;
    let all = inner.view.unconfirmed(space).unwrap();
    let frames = all.iter().map(|c| c.bytes().to_vec()).collect();
    let expected: Vec<Vec<Vec<u8>>> = wire::batches(frames);
    assert!(expected.len() > 4, "the changes fill pushes by their bytes");

    let through = pages::through(&mut inner, space).unwrap();
    let (mut after, mut pushed) = (0, Vec::new());
    loop {
        let page = pages::next(&mut inner, space, after, through).unwrap();
        for (ids, frames) in page.pushes {
            assert_eq!(ids.len(), frames.len());
            pushed.push(frames);
        }
        if page.last {
            break;
        }
        after = page.after;
    }
    assert_eq!(pushed, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_longer_than_a_push_reaches_the_relay_in_local_order() {
    let relay = LocalRelay::start().await;
    relay.proxy.cut();
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let ids = burst(&space).await;
    eventually("offline", || {
        db.sync().current().connection == Connection::Offline
    })
    .await;

    relay.proxy.restore();
    let status = synced(&db).await;

    let log = relay.log(space.id().0).await;
    assert_eq!(
        &log[2..],
        &ids[..],
        "every write after the genesis and envelope"
    );
    assert_eq!(status.unconfirmed, 0);
}
