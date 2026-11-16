//! A reference into another space (RFC 0009, "The Rust SDK"; Carsten,
//! 2026-11-16, NOC-354): written with `Value::from(Reference)`, read with
//! `get::<Reference>`, cleared with `Value::clear()`.

mod support;

use nocentraldb::{ErrorCode, Reference, Value};
use support::{now, open};

#[test]
fn a_reference_is_written_and_read_back_as_its_space_and_id() {
    let db = open();
    let customers = now(db.spaces().create()).unwrap();
    let first = now(customers.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];
    let second = now(customers.create("projects", [("name", Value::from("b"))]))
        .unwrap()
        .ids[0];
    let orders = now(db.spaces().create()).unwrap();

    let task = now(orders.create(
        "tasks",
        [(
            "customer",
            Value::from(Reference::new(customers.id(), first)),
        )],
    ))
    .unwrap()
    .ids[0];
    let read = now(orders.get("tasks", task)).unwrap().unwrap();
    let customer = read
        .get::<Reference>("customer")
        .unwrap()
        .expect("a reference");
    assert_eq!(customer, Reference::new(customers.id(), first));
    assert_eq!((customer.space, customer.id), (customers.id(), first));

    // An update moves the reference; a clear empties it.
    now(orders.update(
        "tasks",
        task,
        [(
            "customer",
            Value::from(Reference::new(customers.id(), second)),
        )],
    ))
    .unwrap();
    let read = now(orders.get("tasks", task)).unwrap().unwrap();
    assert_eq!(
        read.get::<Reference>("customer").unwrap(),
        Some(Reference::new(customers.id(), second))
    );
    now(orders.update("tasks", task, [("customer", Value::clear())])).unwrap();
    let read = now(orders.get("tasks", task)).unwrap().unwrap();
    assert_eq!(read.get::<Reference>("customer").unwrap(), None);
}

#[test]
fn a_reference_field_takes_no_object_id_and_a_link_takes_no_reference() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let target = now(project.create("projects", [("name", Value::from("a"))]))
        .unwrap()
        .ids[0];

    let bare = now(project.create("tasks", [("customer", Value::from(target))]));
    assert_eq!(bare.unwrap_err().code(), ErrorCode::InvalidValue);
    let linked = now(project.create(
        "tasks",
        [("project", Value::from(Reference::new(project.id(), target)))],
    ));
    assert_eq!(linked.unwrap_err().code(), ErrorCode::InvalidValue);
}
