//! Backing off from a relay in trouble (RFC 0007, "Liveness" and "The
//! errors"): a session that makes no progress does not reset the
//! backoff, `rate_limited` waits its retry-after, and a local write does
//! not cut a backoff short.

use std::time::Duration;

use super::fake::{FakeRelay, Trouble};
use super::support::*;
use crate::value::Value;

const ME: Who = 1;

/// A database of `ME` with its own space, syncing with `relay`, that
/// writes every 50 ms for `span`.
async fn write_for(relay: &FakeRelay, span: Duration) -> crate::Db {
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    let start = std::time::Instant::now();
    while start.elapsed() < span {
        space
            .write(|tx| {
                tx.create("tasks", [("title", Value::from("again"))])?;
                Ok(())
            })
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    db
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_that_answers_unavailable_is_retried_with_a_growing_backoff() {
    let relay = FakeRelay::start(Trouble::Unavailable).await;

    let _db = write_for(&relay, Duration::from_secs(2)).await;

    // 100 ms doubling, with jitter: at most 6 attempts in 2 seconds.
    let connections = relay.connections();
    assert!(connections <= 6, "{connections} connections in 2 s");
}

#[tokio::test(flavor = "multi_thread")]
async fn rate_limited_waits_its_retry_after() {
    let relay = FakeRelay::start(Trouble::RateLimited(2)).await;

    let _db = write_for(&relay, Duration::from_millis(1500)).await;

    let connections = relay.connections();
    assert_eq!(connections, 1, "{connections} connections in 1.5 s");
}
