//! A write goes through the member log and the fold into the view, so a
//! read after the write sees it (RFC 0009, "Write" and "Get by id"; RFC
//! 0006, "The member's own writes").

mod support;

use nocentraldb::Value;
use support::{now, open};

#[test]
fn a_task_written_into_a_new_space_reads_back_by_its_id() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();

    let written = now(project.write(|tx| {
        tx.create(
            "tasks",
            [("title", Value::from("Ship the SDK")), ("priority", Value::from(2))],
        )?;
        Ok(())
    }))
    .unwrap();

    assert_eq!(written.ids.len(), 1);
    let task = now(project.get("tasks", written.ids[0])).unwrap().unwrap();
    assert_eq!(task.id(), written.ids[0]);
    assert_eq!(task.space(), project.id());
    assert_eq!(task.get::<&str>("title").unwrap(), Some("Ship the SDK"));
    assert_eq!(task.get::<i64>("priority").unwrap(), Some(2));
    assert_eq!(task.get::<bool>("urgent").unwrap(), None);
    assert!(!task.deleted());
}

#[test]
fn an_object_the_copy_does_not_hold_reads_none() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("A"))])?;
        Ok(())
    }))
    .unwrap();

    assert!(now(project.get("tasks", written.ids[0])).unwrap().is_none());
}
