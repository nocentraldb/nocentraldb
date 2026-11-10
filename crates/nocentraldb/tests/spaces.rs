//! A space handle by id (RFC 0009, "Spaces"): `db.spaces.get` returns the
//! handle of every space the member holds, and `notFound` for a space the
//! copy does not hold (RFC 0009, "Errors").

mod support;

use nocentraldb::{ErrorCode, Value};
use support::{now, open};

#[test]
fn a_space_the_member_created_is_got_by_its_id_and_written_into() {
    let db = open();
    let created = now(db.spaces().create()).unwrap();

    let project = now(db.spaces().get(created.id())).unwrap();
    let written = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("Ship the SDK"))])?;
        Ok(())
    }))
    .unwrap();

    assert_eq!(project.id(), created.id());
    let task = now(created.get("tasks", written.ids[0])).unwrap().unwrap();
    assert_eq!(task.get::<&str>("title").unwrap(), Some("Ship the SDK"));
}

#[test]
fn a_space_the_copy_does_not_hold_is_not_found() {
    let db = open();
    let elsewhere = now(open().spaces().create()).unwrap();

    let error = now(db.spaces().get(elsewhere.id())).unwrap_err();

    assert_eq!(error.code(), ErrorCode::NotFound);
}

#[test]
fn writes_after_one_another_through_two_handles_both_land() {
    let db = open();
    let created = now(db.spaces().create()).unwrap();
    let again = now(db.spaces().get(created.id())).unwrap();

    let first = now(created.write(|tx| {
        tx.create("tasks", [("title", Value::from("one"))])?;
        Ok(())
    }))
    .unwrap();
    let second = now(again.write(|tx| {
        tx.create("tasks", [("title", Value::from("two"))])?;
        Ok(())
    }))
    .unwrap();

    assert_ne!(first.change, second.change);
    assert!(now(again.get("tasks", first.ids[0])).unwrap().is_some());
    assert!(now(created.get("tasks", second.ids[0])).unwrap().is_some());
}
