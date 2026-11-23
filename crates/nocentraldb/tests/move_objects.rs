//! Moving objects to another space (RFC 0016, "What a move is", "The SDK
//! call and its notice"; NOC-475): `Space::move_objects` writes each object,
//! with the same id and its current values, into the destination, then the
//! move into this space. The old space keeps a trace that reads as moved and
//! shows no fields; a write to it is refused with `Moved`. A link to an
//! object left behind is cleared in the new space and counted. A refused
//! move writes nothing. Personal fields in a move come later (NOC-477), so
//! an object that holds a personal value is refused for now.

mod support;

use nocentraldb::{Db, ErrorCode, NoticeCode, Object, ObjectId, OpenOptions, Space, Value};
use support::{LOCK, now};

fn open(lock: &str) -> Db {
    now(Db::open(OpenOptions::new(lock).in_memory())).unwrap()
}

fn read(space: &Space, collection: &str, id: ObjectId) -> Object {
    now(space.get(collection, id)).unwrap().unwrap()
}

/// A project and a task of it.
fn project_and_task(space: &Space) -> (ObjectId, ObjectId) {
    let written = now(space.write(|tx| {
        let project = tx.create("projects", [("name", Value::from("Launch"))])?;
        tx.create(
            "tasks",
            [
                ("title", Value::from("Write the notes")),
                ("status", Value::from("open")),
                ("priority", Value::from(2)),
                ("tags", Value::add(["docs", "release"])),
                ("project", Value::from(project)),
            ],
        )?;
        Ok(())
    }))
    .unwrap();
    (written.ids[0], written.ids[1])
}

#[test]
fn moved_objects_keep_their_ids_and_values_in_the_new_space() {
    let db = open(LOCK);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&personal);

    let moved = now(personal.move_objects(
        &[read(&personal, "projects", p), read(&personal, "tasks", t)],
        &shared,
    ))
    .unwrap();

    assert_eq!(moved.notice.code, NoticeCode::ObjectMoved);
    assert_eq!(moved.ids, [p, t]);
    assert_eq!(moved.cleared_links, 0);
    assert!(moved.conflicts.is_empty());
    let task = read(&shared, "tasks", t);
    assert_eq!(task.space(), shared.id());
    assert_eq!(task.moved_to(), None);
    assert_eq!(task.get::<&str>("title").unwrap(), Some("Write the notes"));
    assert_eq!(task.get::<&str>("status").unwrap(), Some("open"));
    assert_eq!(task.get::<i64>("priority").unwrap(), Some(2));
    assert_eq!(task.get::<&[Value]>("tags").unwrap().unwrap().len(), 2);
    // A link between objects moved together stays valid.
    assert_eq!(task.get::<ObjectId>("project").unwrap(), Some(p));
    assert_eq!(
        read(&shared, "projects", p).get::<&str>("name").unwrap(),
        Some("Launch")
    );
}

#[test]
fn the_old_space_keeps_a_trace_with_no_fields() {
    let db = open(LOCK);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&personal);

    now(personal.move_objects(&[read(&personal, "tasks", t)], &shared)).unwrap();

    let trace = read(&personal, "tasks", t);
    assert_eq!(trace.moved_to(), Some(shared.id()));
    assert_eq!(trace.get::<&str>("title").unwrap(), None);
    assert_eq!(trace.get::<&str>("status").unwrap(), None);
}

#[test]
fn a_link_to_an_object_left_behind_is_cleared_and_counted() {
    let db = open(LOCK);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&personal);

    let moved = now(personal.move_objects(&[read(&personal, "tasks", t)], &shared)).unwrap();

    assert_eq!(moved.cleared_links, 1);
    assert_eq!(
        read(&shared, "tasks", t)
            .get::<ObjectId>("project")
            .unwrap(),
        None
    );
}

#[test]
fn a_write_to_a_moved_object_is_refused_with_moved() {
    let db = open(LOCK);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (_, t) = project_and_task(&personal);
    now(personal.move_objects(&[read(&personal, "tasks", t)], &shared)).unwrap();

    let refused =
        now(personal.write(|tx| tx.update("tasks", t, [("title", Value::from("x"))]))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Moved);
    assert!(refused.to_string().contains(&shared.id().to_string()));
    // The object lives on in the new space, where a write goes through.
    now(shared.write(|tx| tx.update("tasks", t, [("title", Value::from("x"))]))).unwrap();
}

#[test]
fn a_refused_move_writes_nothing() {
    let db = open(LOCK);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let other = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&personal);
    let (_, elsewhere) = project_and_task(&other);
    now(personal.move_objects(&[read(&personal, "projects", p)], &shared)).unwrap();

    let task = read(&personal, "tasks", t);
    for (objects, destination, why) in [
        (
            vec![read(&other, "tasks", elsewhere)],
            &shared,
            "of another space",
        ),
        (
            vec![read(&personal, "projects", p)],
            &shared,
            "already moved",
        ),
        (vec![task.clone(), task.clone()], &shared, "named twice"),
        (vec![task.clone()], &personal, "into its own space"),
    ] {
        let refused = now(personal.move_objects(&objects, destination)).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{why}");
    }
    // Neither space changed: the task is still live where it was, and
    // neither the shared space nor this one holds a copy of the other's.
    assert_eq!(read(&personal, "tasks", t).moved_to(), None);
    assert!(now(shared.get("tasks", t)).unwrap().is_none());
    assert!(now(shared.get("tasks", elsewhere)).unwrap().is_none());
    assert!(now(personal.get("tasks", elsewhere)).unwrap().is_none());
}

#[test]
fn an_object_with_a_personal_value_cannot_be_moved_yet() {
    // `tasks.title` is personal to the task's project (NOC-477).
    let lock = LOCK.replacen(
        r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        r#""title": { "personal": { "person": "project" }, "rule": 1, "since": 1, "type": "ncdb/string" }"#,
        1,
    );
    assert!(lock.contains(r#""person": "project""#));
    let db = open(&lock);
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_task(&personal);

    let refused = now(personal.move_objects(
        &[read(&personal, "projects", p), read(&personal, "tasks", t)],
        &shared,
    ))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    assert!(read(&personal, "tasks", t).moved_to().is_none());
}
