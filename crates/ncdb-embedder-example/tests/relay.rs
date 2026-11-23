//! An app's own tests against a real relay, as an app or engine that embeds
//! the Rust SDK writes them (NOC-507): through the public SDK `nocentraldb`
//! and the test relay `ncdb_relay_server::local::TestRelay` alone.
//!
//! A relay lets only the accounts it lists create spaces, by the account's
//! identity. An app learns its identity from its database, and the database
//! learns the relay's URL when it opens. So the test opens its database
//! once without a relay to read the identity, starts the relay that lists
//! it, and opens the database again with the relay. A database in memory
//! would be a new account on its second open, so it lives in a temporary
//! directory, with its keys in a file there.

use std::future::poll_fn;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use futures_core::Stream;
use nocentraldb::{AddMember, Connection, Db, OpenOptions, Role, Value};
use ncdb_relay_server::local::{Options, TestRelay};

/// The model of the example task list, `apps/tasks-rust/model.ts`.
const LOCK: &str = include_str!("../../../apps/tasks-rust/model.lock.json");

/// How long the test waits for the relay.
const WAIT: Duration = Duration::from_secs(20);

/// A directory of its own under the system's temporary directory, removed
/// with everything in it when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ncdb-embedder-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        TempDir(dir)
    }

    /// The options of the database in this directory, its keys in a file.
    fn options(&self) -> OpenOptions {
        OpenOptions::new(LOCK)
            .path(self.0.join("db"))
            .key_file(self.0.join("keys"))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The identity of the account of the database in `dir`, which the first
/// open creates.
async fn identity(dir: &TempDir) -> String {
    let db = Db::open(dir.options()).await.expect("the database opens");
    db.account().identity().expect("an account")
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
    let (alice_dir, bob_dir) = (TempDir::new(), TempDir::new());
    let relay = TestRelay::start(Options::new().allow_identity(&identity(&alice_dir).await))
        .expect("the relay starts");
    let alice = Db::open(alice_dir.options().relay(&relay.url()))
        .await
        .expect("Alice's database opens");
    let bob = Db::open(bob_dir.options().relay(&relay.url()))
        .await
        .expect("Bob's database opens");

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
                [("title", Value::from("buy milk")), ("status", "open".into())],
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
