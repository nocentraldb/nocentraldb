//! Subscriptions re-run a query when a fold touches it (RFC 0009,
//! "Subscribe"): the stream yields the query's page at once, and again
//! after a fold that touched an object of the query's collection, the whole
//! page each time. A stream that waits has no page ready; dropping it stops
//! it.

mod support;

use nocentraldb::{Page, Space, Value};
use support::{now, open, ready};

fn write_task(space: &Space, title: &str, priority: i64) {
    now(space.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from(title)),
                ("status", Value::from("open")),
                ("priority", Value::from(priority)),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
}

fn titles(page: Page) -> Vec<String> {
    page.items
        .iter()
        .map(|item| item.get::<&str>("title").unwrap().unwrap().to_owned())
        .collect()
}

#[test]
fn db_subscribe_yields_the_page_at_once_and_again_after_a_write() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    write_task(&project, "first", 2);
    let query = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"]);

    let mut live = db.subscribe(query);
    assert_eq!(titles(ready(&mut live).unwrap().unwrap()), ["first"]);
    assert!(ready(&mut live).is_none());

    write_task(&project, "second", 1);
    assert_eq!(
        titles(ready(&mut live).unwrap().unwrap()),
        ["second", "first"]
    );
    assert!(ready(&mut live).is_none());
}

#[test]
fn writes_before_the_next_poll_yield_one_page() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let query = db.index("tasks", "byPriority").unwrap();
    let mut live = db.subscribe(query);
    assert!(ready(&mut live).unwrap().unwrap().items.is_empty());

    write_task(&project, "one", 1);
    write_task(&project, "two", 2);

    assert_eq!(titles(ready(&mut live).unwrap().unwrap()), ["one", "two"]);
    assert!(ready(&mut live).is_none());
}

#[test]
fn a_write_to_another_collection_does_not_wake_it() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let mut live = db.subscribe(db.index("tasks", "byPriority").unwrap());
    ready(&mut live).unwrap().unwrap();

    now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("Home"))])?;
        Ok(())
    }))
    .unwrap();

    assert!(ready(&mut live).is_none());
}

#[test]
fn space_subscribe_follows_a_query_in_its_space_only() {
    let db = open();
    let a = now(db.spaces().create()).unwrap();
    let b = now(db.spaces().create()).unwrap();
    let query = db.index("tasks", "open").unwrap().equal(["open"]);
    let mut live = a.subscribe(query);
    assert!(ready(&mut live).unwrap().unwrap().items.is_empty());

    write_task(&b, "elsewhere", 1);
    assert!(ready(&mut live).is_none());

    write_task(&a, "here", 2);
    assert_eq!(titles(ready(&mut live).unwrap().unwrap()), ["here"]);
}

#[test]
fn a_dropped_subscription_stops_and_writes_go_on() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let live = db.subscribe(db.index("tasks", "byPriority").unwrap());
    drop(live);

    write_task(&project, "after", 1);

    let page = now(db.find(db.index("tasks", "byPriority").unwrap())).unwrap();
    assert_eq!(titles(page), ["after"]);
}
