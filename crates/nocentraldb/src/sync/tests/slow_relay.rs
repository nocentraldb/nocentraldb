//! A relay that answers late, as one does on a loaded machine, is still
//! a relay (NOC-331; RFC 0007, "Liveness"): a member that waits the
//! bounded time for each answer keeps its connection and syncs, however
//! slowly. Only a relay that does not answer within the bound is lost
//! (`timeouts.rs`).

use std::time::Duration;

use super::support::*;
use crate::value::Value;

const ME: Who = 1;

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_that_answers_late_but_in_time_keeps_its_member_in_sync() {
    let relay = LocalRelay::start().await;
    let db = open(ME, &relay.url()).await;
    let project = db.spaces().create().await.unwrap();
    synced(&db).await;

    // Each way 400 ms: every answer arrives 800 ms after its request.
    relay.proxy.slow(Duration::from_millis(400));
    project
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("written while slow"))])?;
            Ok(())
        })
        .await
        .unwrap();
    synced(&db).await;
}
