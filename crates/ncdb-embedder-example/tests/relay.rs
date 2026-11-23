//! An app's own tests against a real relay, as an app or engine that embeds
//! the Rust SDK writes them (NOC-507): through the public SDK `nocentraldb`
//! and the test relay `ncdb_relay_server::local::TestRelay` alone.
//!
//! A relay lets only the accounts it admits create spaces. The test relay
//! started with `Options::admit_any` signs an admission for any account,
//! and each database fetches its own through `OpenOptions::admission`, so
//! a test opens each database once, in memory, with the relay.

use std::future::poll_fn;
use std::pin::Pin;
use std::time::Duration;

use futures_core::Stream;
use ncdb_relay_server::local::{Options, TestRelay};
use nocentraldb::{AddMember, Connection, Db, OpenOptions, Role, Value};

/// The model of the example task list, `apps/tasks-rust/model.ts`.
const LOCK: &str = include_str!("../../../apps/tasks-rust/model.lock.json");

/// How long the test waits for the relay.
const WAIT: Duration = Duration::from_secs(20);

/// A database in memory that syncs through `relay` and fetches its
/// admissions from it.
async fn open(relay: &TestRelay) -> Db {
    let options = OpenOptions::new(LOCK)
        .in_memory()
        .relay(&relay.url())
        .admission(relay.admissions());
    Db::open(options).await.expect("the database opens")
}

/// The next item of a stream, within [`WAIT`].
async fn next<S: Stream + Unpin>(stream: &mut S) -> S::Item {
    let item = poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx));
    tokio::time::timeout(WAIT, item)
        .await
        .expect("in time")
        .expect("the stream does not end")
}

/// Waits until `db` is connected and the relay holds every change it wrote.
async fn synced(db: &Db) {
    let mut sync = db.sync().subscribe();
    loop {
        let status = next(&mut sync).await;
        assert_eq!(status.error, None, "the relay reports an error");
        if status.connection == Connection::Connected && status.unconfirmed == 0 {
            return;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_write_on_one_member_reaches_another_after_sync() {
    let relay = TestRelay::start(Options::new().admit_any()).expect("the relay starts");
    let alice = open(&relay).await;
    let bob = open(&relay).await;

    let space = alice.spaces().create().await.expect("a space");
    let invitation = bob.invitation().await;
    let options = AddMember {
        role: Role::Writer,
        // In an app, a person compares the fingerprint on both devices.
        fingerprint: Some(invitation.fingerprint()),
    };
    space
        .members()
        .add(&invitation, options)
        .await
        .expect("Bob is added");
    space
        .write(|tx| {
            tx.create(
                "tasks",
                [
                    ("title", Value::from("buy milk")),
                    ("status", "open".into()),
                ],
            )?;
            Ok(())
        })
        .await
        .expect("the write");
    synced(&alice).await;

    let query = bob.index("tasks", "byStatus").expect("the index").take(10);
    let mut pages = bob.subscribe(query);
    loop {
        let page = next(&mut pages).await.expect("a page");
        let titles: Vec<_> = page
            .items
            .iter()
            .map(|task| task.get::<&str>("title").unwrap().unwrap_or("").to_owned())
            .collect();
        if titles == ["buy milk"] {
            break;
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_that_does_not_list_the_space_creator_refuses_with_not_allowed() {
    let relay = TestRelay::start(Options::new()).expect("the relay starts");
    let db = Db::open(OpenOptions::new(LOCK).in_memory().relay(&relay.url()))
        .await
        .expect("the database opens");

    let space = db.spaces().create().await.expect("a space, on this device");

    let mut statuses = space.status().subscribe();
    loop {
        let status = next(&mut statuses).await;
        if let Some(refused) = status.refused.first() {
            assert_eq!(refused.code, "not_allowed");
            break;
        }
    }
}
