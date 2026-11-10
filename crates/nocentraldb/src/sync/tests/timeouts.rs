//! A relay that stops answering is a lost connection (RFC 0007,
//! "Liveness"): the WebSocket handshake, every request and every ping
//! wait a bounded time. The bounds are short in these tests.

use std::time::Duration;

use super::fake::{FakeRelay, Trouble};
use super::support::*;

const ME: Who = 1;

/// How many connections `trouble` gets from a database with a space of
/// its own within 3 seconds.
async fn connections(trouble: Trouble) -> usize {
    let relay = FakeRelay::start(trouble).await;
    let db = open(ME, &relay.url()).await;
    db.spaces().create().await.unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    relay.connections()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_handshake_that_never_answers_times_out() {
    let n = connections(Trouble::Hang).await;
    assert!(n >= 2, "{n} connections: the first one never timed out");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_is_never_answered_times_out() {
    let n = connections(Trouble::Silent).await;
    assert!(n >= 2, "{n} connections: the first one never timed out");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_ping_without_a_pong_is_a_lost_connection() {
    let n = connections(Trouble::NoPong).await;
    assert!(n >= 2, "{n} connections: the first one never timed out");
}
