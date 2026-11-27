//! A reference into another space (RFC 0009, "The Rust SDK"; Carsten,
//! 2026-11-16, NOC-354): written with `Value::from(Reference)`, read with
//! `get::<Reference>`, cleared with `Value::clear()`. The other forms a
//! reference field takes, and a reference in a link field, are in
//! `write_reference_forms.rs` (RFC 0018, "One function, two cases").

mod support;

use nocentraldb::{Reference, Value};
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
