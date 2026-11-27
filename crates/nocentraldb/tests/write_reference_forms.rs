//! The forms a reference field takes, and a reference in a link field (RFC
//! 0018, "One function, two cases" and "Existing link fields read as
//! references"): a reference field takes a [`Reference`] or a bare object
//! id, which names the written object's own space; a link field takes a
//! reference into its own space, and refuses one into another space with
//! `InvalidValue`, naming the field. `Reference::from(&Object)` makes a
//! reference of an object the SDK read (NOC-573).

mod support;

use nocentraldb::{ErrorCode, ObjectId, Reference, Value};
use support::{now, open};

#[test]
fn a_bare_object_id_in_a_reference_field_names_the_objects_own_space() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let target = now(space.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let task = now(space.create("tasks", [("customer", Value::from(target))]))
        .unwrap()
        .ids[0];
    let read = now(space.get("tasks", task)).unwrap().unwrap();
    assert_eq!(
        read.get::<Reference>("customer").unwrap(),
        Some(Reference::new(space.id(), target))
    );
}

#[test]
fn a_link_field_takes_a_reference_into_its_own_space() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let target = now(space.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let task = now(space.create(
        "tasks",
        [("project", Value::from(Reference::new(space.id(), target)))],
    ))
    .unwrap()
    .ids[0];
    let read = now(space.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<ObjectId>("project").unwrap(), Some(target));
}

#[test]
fn a_link_field_refuses_a_reference_into_another_space_naming_the_field() {
    let db = open();
    let customers = now(db.spaces().create()).unwrap();
    let target = now(customers.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let orders = now(db.spaces().create()).unwrap();
    let error = now(orders.create(
        "tasks",
        [(
            "project",
            Value::from(Reference::new(customers.id(), target)),
        )],
    ))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidValue);
    assert!(error.to_string().contains("project"), "{error}");
}

#[test]
fn a_reference_from_an_object_the_sdk_read_names_its_space_and_id() {
    // NOC-573 (Carsten, 2026-11-27): `Reference::from(&Object)` takes the
    // space and id of an object the SDK read, and nothing else of it.
    let db = open();
    let customers = now(db.spaces().create()).unwrap();
    let target = now(customers.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let project = now(customers.get("projects", target)).unwrap().unwrap();
    assert_eq!(
        Reference::from(&project),
        Reference::new(customers.id(), target)
    );

    let orders = now(db.spaces().create()).unwrap();
    let task = now(orders.create(
        "tasks",
        [("customer", Value::from(Reference::from(&project)))],
    ))
    .unwrap()
    .ids[0];
    let read = now(orders.get("tasks", task)).unwrap().unwrap();
    assert_eq!(
        read.get::<Reference>("customer").unwrap(),
        Some(Reference::new(customers.id(), target))
    );
}
