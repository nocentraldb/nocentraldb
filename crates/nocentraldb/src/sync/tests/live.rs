//! Live entries (RFC 0007, "Subscribe"): the member folds those of the
//! spaces it subscribed to in this session, and ignores those of any
//! other space, which a relay should never send it.

use std::time::Duration;

use super::fake::{FakeRelay, Trouble, stranger};
use super::support::*;

const ME: Who = 1;

#[tokio::test(flavor = "multi_thread")]
async fn live_entries_of_a_space_not_subscribed_to_are_ignored() {
    let relay = FakeRelay::start(Trouble::Stranger).await;
    let db = open(ME, &relay.url()).await;

    eventually("connected", || relay.connections() >= 1).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let inner = crate::db::lock(&db.inner);
    assert_eq!(inner.sync_folds(), 0, "the stranger's genesis was folded");
    assert!(
        !inner.spaces.contains_key(&stranger().space),
        "the stranger's space became one the database syncs"
    );
}
