//! The first end-to-end slice (NOC-66, NOC-91): two devices share a space
//! through the relay server on SQLite, run locally, and end with the same
//! view. Device A creates the space and adds device B as a writer with a
//! key envelope (RFC 0008, "Adding a member"; RFC 0003, "Sharing a space
//! key with a new member"); both write, also concurrently into one object
//! while the network is cut; both sync (RFC 0007), and their reads through
//! the SDK compare equal (GENESIS.md, "Convergence").
//!
//! A adds B with `space.members().add` and B's invitation (RFC 0009,
//! "Invitations and members"). Both devices are of the simulated account,
//! so A adds B without a fingerprint.

use ncdb_core::ids::ChangeId;

use super::support::*;
use crate::db::{Db, lock};
use crate::id::{ObjectId, SpaceId};
use crate::members::{AddMember, Role};
use crate::object::Object;
use crate::query::Page;
use crate::space::Space;
use crate::value::Value;

/// Device A, which creates the space.
const A: Who = 1;
/// Device B, which A adds.
const B: Who = 2;

/// Whether the database's copy of `space` holds every change of the
/// relay's log of it, `log`.
fn holds(db: &Db, space: SpaceId, log: &[ChangeId]) -> bool {
    let inner = lock(&db.inner);
    log.iter()
        .all(|id| inner.view.relay_position(space.0, *id).is_some())
}

/// What the device reads of the shared space through the SDK: every task
/// by priority, and each of `ids` by id.
fn read(db: &Db, project: &Space, ids: &[ObjectId]) -> (Page, Vec<Option<Object>>) {
    let page = now(db.find(db.index("tasks", "byPriority").unwrap().take(50))).unwrap();
    let objects = ids
        .iter()
        .map(|id| now(project.get("tasks", *id)).unwrap())
        .collect();
    (page, objects)
}

#[tokio::test(flavor = "multi_thread")]
async fn two_devices_share_a_space_through_the_relay_and_see_the_same_view() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open(B, &relay.url()).await;

    let on_a = now(a.spaces().create()).unwrap();
    let shared = now(on_a.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from("from A")),
                ("status", Value::from("open")),
                ("priority", Value::from(1)),
                ("tags", Value::add(["a"])),
            ],
        )?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let invitation = now(b.invitation());
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(on_a.members().add(&invitation, writer)).unwrap();
    synced(&a).await;

    // B stays connected: the relay tells it that it was added (RFC 0007,
    // "Subscribe", decision R13).
    let mut on_b = Err(());
    eventually("B holds the space", || {
        on_b = now(b.spaces().get(on_a.id())).map_err(|_| ());
        on_b.is_ok()
    })
    .await;
    let on_b = on_b.unwrap();
    eventually("B holds A's task", || {
        now(on_b.get("tasks", shared)).unwrap().is_some()
    })
    .await;
    let of_b = now(on_b.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from("from B")),
                ("priority", Value::from(2)),
            ],
        )?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    synced(&a).await;
    synced(&b).await;

    // Concurrent writes into one object: neither device sees the other's
    // change before it writes its own.
    relay.proxy.cut();
    now(on_a.write(|tx| {
        tx.update(
            "tasks",
            shared,
            [
                ("title", Value::from("A's title")),
                ("tags", Value::add(["x"])),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
    now(on_b.write(|tx| {
        tx.update(
            "tasks",
            shared,
            [
                ("title", Value::from("B's title")),
                ("tags", Value::add(["y"])),
                ("priority", Value::from(5)),
            ],
        )?;
        tx.update("tasks", of_b, [("status", Value::from("done"))])?;
        Ok(())
    }))
    .unwrap();
    relay.proxy.restore();

    let status_a = synced(&a).await;
    let status_b = synced(&b).await;
    assert_eq!((status_a.refused, status_b.refused), (0, 0));
    let log = relay.log(on_a.id().0).await;
    eventually("both devices hold the relay's whole log", || {
        holds(&a, on_a.id(), &log) && holds(&b, on_a.id(), &log)
    })
    .await;

    let ids = [shared, of_b];
    let (page_a, objects_a) = read(&a, &on_a, &ids);
    let (page_b, objects_b) = read(&b, &on_b, &ids);
    assert_eq!(page_a, page_b);
    assert_eq!(objects_a, objects_b);
    assert_eq!(page_a.items.len(), 2);
    let task = objects_a[0].as_ref().expect("the shared task");
    let title = task.get::<&str>("title").unwrap();
    assert!(
        matches!(title, Some("A's title" | "B's title")),
        "{title:?}"
    );
    assert_eq!(task.get::<i64>("priority").unwrap(), Some(5));
    let tags: Vec<&str> = task
        .get::<&[Value]>("tags")
        .unwrap()
        .expect("tags")
        .iter()
        .map(|tag| <&str>::try_from(tag).unwrap())
        .collect();
    assert_eq!(tags, ["a", "x", "y"]);
}
