//! References in a move (RFC 0018, "When the source or the target moves";
//! RFC 0016, "Links in the moved object"): the move writes each reference
//! among objects moved together with the new space, so it stays in-space.
//! A reference to an object left behind keeps naming the old space and is
//! not cleared, unlike a link, which is cleared and counted.

mod support;

use nocentraldb::{Db, Object, ObjectId, OpenOptions, Reference, Space, Value};
use support::{LOCK, now};

fn open() -> Db {
    now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap()
}

fn read(space: &Space, collection: &str, id: ObjectId) -> Object {
    now(space.get(collection, id)).unwrap().unwrap()
}

/// A project, and a task whose `customer` references it in `space`.
fn project_and_referencing_task(space: &Space) -> (ObjectId, ObjectId) {
    let project = now(space.create("projects", [("name", Value::from("Launch"))]))
        .unwrap()
        .ids[0];
    let task = now(space.create(
        "tasks",
        [("customer", Value::from(Reference::new(space.id(), project)))],
    ))
    .unwrap()
    .ids[0];
    (project, task)
}

#[test]
fn a_reference_among_objects_moved_together_names_the_new_space() {
    let db = open();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_referencing_task(&personal);

    let moved = now(personal.move_objects(
        &[read(&personal, "projects", p), read(&personal, "tasks", t)],
        &shared,
    ))
    .unwrap();

    assert_eq!(moved.cleared_links, 0);
    assert_eq!(
        read(&shared, "tasks", t)
            .get::<Reference>("customer")
            .unwrap(),
        Some(Reference::new(shared.id(), p))
    );
}

#[test]
fn a_reference_to_an_object_left_behind_keeps_the_old_space_and_is_not_cleared() {
    let db = open();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let (p, t) = project_and_referencing_task(&personal);

    let moved = now(personal.move_objects(&[read(&personal, "tasks", t)], &shared)).unwrap();

    assert_eq!(moved.cleared_links, 0);
    assert_eq!(
        read(&shared, "tasks", t)
            .get::<Reference>("customer")
            .unwrap(),
        Some(Reference::new(personal.id(), p))
    );
}

#[test]
fn a_reference_into_a_third_space_stays_as_written() {
    let db = open();
    let personal = now(db.spaces().create()).unwrap();
    let shared = now(db.spaces().create()).unwrap();
    let elsewhere = now(db.spaces().create()).unwrap();
    let p = now(elsewhere.create("projects", [("name", Value::from("X"))]))
        .unwrap()
        .ids[0];
    let t = now(personal.create(
        "tasks",
        [("customer", Value::from(Reference::new(elsewhere.id(), p)))],
    ))
    .unwrap()
    .ids[0];

    now(personal.move_objects(&[read(&personal, "tasks", t)], &shared)).unwrap();

    assert_eq!(
        read(&shared, "tasks", t)
            .get::<Reference>("customer")
            .unwrap(),
        Some(Reference::new(elsewhere.id(), p))
    );
}
