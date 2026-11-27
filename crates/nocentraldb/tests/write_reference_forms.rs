//! The forms a reference field takes, and a reference in a link field (RFC
//! 0018, "One function, two cases" and "Existing link fields read as
//! references"): a reference field takes a [`Reference`] or a bare object
//! id, which names the written object's own space; a link field takes a
//! reference into its own space, and refuses one into another space with
//! `InvalidValue`, naming the field.

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
