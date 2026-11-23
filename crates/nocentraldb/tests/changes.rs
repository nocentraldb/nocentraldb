//! The change event (RFC 0009, "Subscribe", `db.changes.subscribe`):
//! `db.changes().subscribe()` yields, after each fold, one `Change` per
//! collection the fold touched, with its space, the ids and whether it was
//! this member's own write. It carries ids, never values. A change that
//! sync brings yields `Origin::Remote`; the sync tests cover it
//! (`src/sync/tests/pull.rs`).

mod support;

use nocentraldb::{Origin, Value};
use support::{now, open, ready};

#[test]
fn a_local_write_yields_its_space_collection_ids_and_origin_local() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let mut changes = db.changes().subscribe();
    assert!(ready(&mut changes).is_none(), "nothing folded yet");

    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("a"))])?;
        tx.create("tasks", [("title", Value::from("b"))])?;
        Ok(())
    }))
    .unwrap();

    let change = ready(&mut changes).expect("the write's fold");
    assert_eq!(change.space, project.id());
    assert_eq!(change.collection, "tasks");
    assert_eq!(change.origin, Origin::Local);
    let mut ids = change.ids.clone();
    ids.sort();
    let mut expected = written.ids.clone();
    expected.sort();
    assert_eq!(ids, expected);
    assert!(ready(&mut changes).is_none(), "one fold, one item");
}

#[test]
fn one_fold_that_touches_two_collections_yields_one_item_per_collection() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let mut changes = db.changes().subscribe();

    now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("a"))])?;
        tx.create("projects", [("name", Value::from("p"))])?;
        Ok(())
    }))
    .unwrap();

    let mut collections = Vec::new();
    while let Some(change) = ready(&mut changes) {
        assert_eq!(change.space, project.id());
        assert_eq!(change.origin, Origin::Local);
        assert_eq!(change.ids.len(), 1, "{change:?}");
        collections.push(change.collection);
    }
    collections.sort();
    assert_eq!(collections, ["projects", "tasks"]);
}

#[test]
fn folds_before_the_next_poll_each_yield_their_items_in_order() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let mut changes = db.changes().subscribe();

    let first = now(project.create("tasks", [("title", Value::from("a"))])).unwrap();
    let second = now(project.create("projects", [("name", Value::from("p"))])).unwrap();

    let a = ready(&mut changes).unwrap();
    let b = ready(&mut changes).unwrap();
    assert_eq!((a.collection.as_str(), a.ids.clone()), ("tasks", first.ids));
    assert_eq!(
        (b.collection.as_str(), b.ids.clone()),
        ("projects", second.ids)
    );
    assert!(ready(&mut changes).is_none());
}

#[test]
fn a_dropped_subscription_stops_and_writes_go_on() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    drop(db.changes().subscribe());
    now(project.create("tasks", [("title", Value::from("a"))])).unwrap();
}
