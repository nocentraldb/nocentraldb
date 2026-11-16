//! A large change whose upload S3 refuses (NOC-348; Carsten, 2026-11-16,
//! NOC-349, option A): the connection stays up, `SyncStatus::error` shows
//! `upload_failed`, and the change stays unconfirmed and goes up again
//! after a backoff. Before, a refused PUT ended the connection, and the
//! member reconnected and asked for a new upload URL again and again.

use super::fake::{FakeRelay, Trouble};
use super::support::*;
use crate::sync::Connection;
use crate::value::Value;

const ME: Who = 1;

/// A title of 30 KB, as in the owner's smoke test: its change does not
/// fit into one message.
fn large_title() -> String {
    "x".repeat(30 * 1024)
}

/// `db` writes a large change into a new space.
async fn write_large(db: &crate::Db) {
    let space = db.spaces().create().await.unwrap();
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from(large_title().as_str()))])?;
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_upload_shows_upload_failed_and_keeps_the_connection() {
    let relay = FakeRelay::start(Trouble::RefuseUploads(usize::MAX)).await;
    let db = open(ME, &relay.url()).await;
    write_large(&db).await;
    eventually("S3 refused the upload twice", || relay.puts() >= 2).await;
    let status = db.sync().current();
    assert_eq!(status.error.as_deref(), Some("upload_failed"));
    assert_eq!(status.connection, Connection::Connected);
    assert!(status.unconfirmed > 0, "the refused change is unconfirmed");
    assert_eq!(relay.connections(), 1, "the member reconnected");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_that_goes_through_later_clears_upload_failed() {
    let relay = FakeRelay::start(Trouble::RefuseUploads(1)).await;
    let db = open(ME, &relay.url()).await;
    write_large(&db).await;
    let status = synced(&db).await;
    assert_eq!(status.error, None);
    assert_eq!(relay.puts(), 2);
    assert_eq!(relay.connections(), 1, "the member reconnected");
}
