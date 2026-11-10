//! The calls on a transaction and what each field type takes (RFC 0009,
//! "Field operations per field type"): an update writes fields of an object
//! the copy holds, a set is written with `Value::add` and `Value::remove`,
//! a link takes an object id, a delete marks the object deleted; a field
//! written twice in one transaction keeps its last value.

mod support;

use nocentraldb::{ObjectId, Space, Value};
use support::{now, open};

fn create(project: &Space, collection: &str, values: Vec<(&str, Value)>) -> ObjectId {
    let written = now(project.write(|tx| {
        tx.create(collection, values)?;
        Ok(())
    }))
    .unwrap();
    written.ids[0]
}

#[test]
fn an_update_writes_fields_of_an_object_the_copy_holds() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let task = create(&project, "tasks", vec![("title", Value::from("Draft"))]);

    now(project.write(|tx| {
        tx.update(
            "tasks",
            task,
            [
                ("title", Value::from("Ship")),
                ("status", Value::from("done")),
                ("estimate", Value::from(1.5)),
                ("urgent", Value::from(true)),
            ],
        )
    }))
    .unwrap();

    let read = now(project.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<&str>("title").unwrap(), Some("Ship"));
    assert_eq!(read.get::<&str>("status").unwrap(), Some("done"));
    assert_eq!(read.get::<f64>("estimate").unwrap(), Some(1.5));
    assert_eq!(read.get::<bool>("urgent").unwrap(), Some(true));
}

#[test]
fn a_set_takes_additions_and_removals_and_reads_as_its_elements() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let task = create(
        &project,
        "tasks",
        vec![("tags", Value::add(["docs", "sdk", "rust"]))],
    );

    now(project.write(|tx| tx.update("tasks", task, [("tags", Value::remove(["sdk"]))]))).unwrap();

    let read = now(project.get("tasks", task)).unwrap().unwrap();
    let tags: Vec<&str> = read
        .get::<&[Value]>("tags")
        .unwrap()
        .unwrap()
        .iter()
        .map(|tag| <&str>::try_from(tag).unwrap())
        .collect();
    // In the order of the elements' canonical bytes.
    assert_eq!(tags, ["docs", "rust"]);
}

#[test]
fn a_link_takes_the_id_of_an_object_of_the_same_space() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        let home = tx.create("projects", [("name", Value::from("Home"))])?;
        tx.create("tasks", [("project", Value::from(home))])?;
        Ok(())
    }))
    .unwrap();

    let task = now(project.get("tasks", written.ids[1])).unwrap().unwrap();
    assert_eq!(task.get::<ObjectId>("project").unwrap(), Some(written.ids[0]));
}

#[test]
fn a_field_written_twice_in_one_transaction_keeps_its_last_value() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        let task = tx.create("tasks", [("priority", Value::from(1))])?;
        tx.update("tasks", task, [("priority", Value::from(3))])
    }))
    .unwrap();

    let task = now(project.get("tasks", written.ids[0])).unwrap().unwrap();
    assert_eq!(task.get::<i64>("priority").unwrap(), Some(3));
}

#[test]
fn a_deleted_object_reads_by_id_as_deleted() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let task = create(&project, "tasks", vec![("title", Value::from("Old"))]);

    now(project.write(|tx| tx.delete("tasks", task))).unwrap();

    let read = now(project.get("tasks", task)).unwrap().unwrap();
    assert!(read.deleted());
    assert_eq!(read.get::<&str>("title").unwrap(), Some("Old"));
}
