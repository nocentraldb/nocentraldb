//! Opening refuses a lock it cannot read (RFC 0009, "Errors":
//! `modelRefused`); one write is one change; and no public type prints
//! content: `Debug` of a value, an object, a query, a cursor and an error
//! shows no field value (RFC 0009, "The Rust SDK"; RFC 0003, "Keys never
//! reach logs, errors or telemetry").

mod support;

use nocentraldb::{Db, ErrorCode, OpenOptions, Value};
use support::{LOCK, now, open};

/// A text no type may print.
const CANARY: &str = "canary-7f3a";

#[test]
fn a_lock_that_does_not_read_is_refused() {
    for lock in ["", "{}", &LOCK.replace("\"format\": 1", "\"format\": 99")] {
        let error = now(Db::open(OpenOptions::new(lock).in_memory())).unwrap_err();
        assert_eq!(error.code(), ErrorCode::ModelRefused);
    }
}

#[test]
fn one_write_is_one_change() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();

    let first = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("A"))])?;
        tx.create("projects", [("name", Value::from("B"))])?;
        tx.create("tasks", [("title", Value::from("C"))])?;
        Ok(())
    }))
    .unwrap();
    let second = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("D"))])?;
        Ok(())
    }))
    .unwrap();

    assert_eq!(first.ids.len(), 3);
    assert_ne!(first.change, second.change);
    assert_eq!(first.change.to_string().len(), 64);
}

#[test]
fn no_public_type_prints_a_field_value() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        tx.create(
            "tasks",
            [
                ("title", Value::from(CANARY)),
                ("status", Value::from("open")),
                ("tags", Value::add([CANARY])),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
    let task = now(project.get("tasks", written.ids[0])).unwrap().unwrap();
    let query = db
        .index("tasks", "byStatusPriority")
        .unwrap()
        .equal(["open"])
        .take(1);
    let page = now(db.find(&query)).unwrap();
    let refused = now(project.write(|tx| {
        tx.create("tasks", [("priority", Value::from(CANARY))])?;
        Ok(())
    }))
    .unwrap_err();

    let shown = format!(
        "{:?} {:?} {:?} {:?} {:?} {refused} {:?} {:?} {:?}",
        Value::from(CANARY),
        task,
        query,
        page,
        refused,
        written,
        project,
        db,
    );
    assert!(!shown.contains(CANARY), "{shown}");
}
