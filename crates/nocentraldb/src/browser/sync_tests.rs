//! Syncing from the browser (NOC-175; NOC-245): two databases in a
//! dedicated worker share a space through a relay over the browser's
//! WebSocket, and a change too large for one message goes up and comes down
//! through the relay's upload and download URLs with `fetch()`, from
//! another origin than the relay's (RFC 0007, "Large changes": CORS for any
//! origin).
//!
//! The relay is the local relay server, which the `sdk-keys` step of
//! browser-view.yml starts at the URL this module is built with,
//! `NCDB_TEST_RELAY`, by default `ws://127.0.0.1:7711`, and which lets the
//! members of [`ACCOUNT`] create spaces. Node runs none of these: they are
//! ignored there, as they need that relay.

use ed25519_dalek::SigningKey;
use wasm_bindgen_test::wasm_bindgen_test as test;

use crate::id::{ObjectId, SpaceId};
use crate::members::{AddMember, Role};
use crate::sync::time::{Instant, sleep};
use crate::sync::{Connection, SyncStatus};
use crate::value::Value;
use crate::{Db, OpenOptions};

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The relay the harness starts.
const RELAY: &str = match option_env!("NCDB_TEST_RELAY") {
    Some(url) => url,
    None => "ws://127.0.0.1:7711",
};

/// The secret of the account whose members may create spaces on the
/// harness's relay; scripts/sdk-keys passes its public key to the relay.
pub(crate) const ACCOUNT: [u8; 32] = [0x41; 32];

/// How long a test waits for syncing.
const WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// A database in memory of a fresh member of [`ACCOUNT`], syncing with the
/// harness's relay.
async fn open() -> Db {
    let (signing, agreement) = (*crate::keys::random(), *crate::keys::random());
    let options = OpenOptions::new(LOCK).in_memory().relay(RELAY);
    Db::open_as(
        options,
        signing,
        agreement,
        SigningKey::from_bytes(&ACCOUNT),
    )
    .await
    .expect("a database with a relay opens in the browser")
}

/// Polls `future`, which must not wait.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// Waits until `done` holds.
async fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < WAIT, "{what}: not in time");
        sleep(std::time::Duration::from_millis(50)).await;
    }
}

/// Waits until `db` is connected and the relay confirmed every change of
/// its member.
async fn synced(db: &Db) -> SyncStatus {
    let mut status = db.sync().current();
    eventually("synced", || {
        status = db.sync().current();
        status.connection == Connection::Connected && status.unconfirmed == 0
    })
    .await;
    status
}

/// The title of `task` as `db` holds it, once it holds the task.
fn title(db: &Db, space: SpaceId, task: ObjectId) -> Option<String> {
    let space = now(db.spaces().get(space)).ok()?;
    let task = now(space.get("tasks", task)).ok()??;
    task.get::<&str>("title").ok()?.map(str::to_owned)
}

/// Device A creates a space and adds device B, both of [`ACCOUNT`]; both
/// synced.
async fn shared() -> (Db, Db, crate::Space) {
    let a = open().await;
    let b = open().await;
    synced(&a).await;
    synced(&b).await;
    let project = now(a.spaces().create()).unwrap();
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(b.invitation()), writer)).unwrap();
    synced(&a).await;
    (a, b, project)
}

/// A task with `title` written by `db` into `space`.
fn task(space: &crate::Space, title: &str) -> ObjectId {
    now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from(title))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0]
}

#[test]
#[ignore = "needs the harness's relay: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn two_databases_share_a_space_through_the_relay_over_the_browsers_websocket() {
    let (a, b, project) = shared().await;
    let written = task(&project, "from the browser");
    let status = synced(&a).await;
    assert_eq!(status.refused, 0, "the relay refused a change");
    eventually("B holds A's task", || {
        title(&b, project.id(), written).as_deref() == Some("from the browser")
    })
    .await;
}

#[test]
#[ignore = "needs the harness's relay: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn a_large_change_goes_up_and_comes_down_through_fetch_from_another_origin() {
    let (a, b, project) = shared().await;
    // A title of 300 KiB: its change does not fit into a 128 KiB message.
    let big = "x".repeat(300 * 1024);
    let written = task(&project, &big);
    let status = synced(&a).await;
    assert_eq!(status.refused, 0, "the relay refused the large change");
    eventually("B holds the large task", || {
        title(&b, project.id(), written).as_deref() == Some(big.as_str())
    })
    .await;
}
