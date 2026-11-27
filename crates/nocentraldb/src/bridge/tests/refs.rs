//! A reference written through the bridge (RFC 0009, "Field operations per
//! field type"; NOC-354): the TypeScript SDK writes `{ space, id }`, the
//! target's space id and object id as a read gives them, or `null` to clear
//! it, and a read gives the reference back as `{ space, id }`.

use ncdb_core::cbor::Value;

use super::fields::{fields, op, opened_with, write};
use super::{Bridge, LOCK, code, field, map, ok, text};

/// A project in a space of its own, as the reference `{ space, id }` to it.
fn customer(bridge: &mut Bridge) -> Value {
    let space = ok(bridge, "spaces.create", map(&[]));
    let id = write(
        bridge,
        &space,
        vec![op("create", "projects", None, map(&[("name", text("c"))]))],
    )
    .remove(0);
    map(&[("space", space), ("id", id)])
}

#[test]
fn a_reference_written_as_space_and_id_reads_back_as_that_reference() {
    let mut bridge = opened_with(LOCK);
    let first = customer(&mut bridge);
    let second = customer(&mut bridge);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let task = write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "tasks",
            None,
            map(&[("customer", first.clone())]),
        )],
    )
    .remove(0);
    assert_eq!(
        field(&fields(&mut bridge, &space, "tasks", &task), "customer"),
        Some(&first)
    );

    // An update moves the reference; null clears it.
    let update = |values| vec![op("update", "tasks", Some(&task), values)];
    write(
        &mut bridge,
        &space,
        update(map(&[("customer", second.clone())])),
    );
    assert_eq!(
        field(&fields(&mut bridge, &space, "tasks", &task), "customer"),
        Some(&second)
    );
    write(
        &mut bridge,
        &space,
        update(map(&[("customer", Value::Null)])),
    );
    let cleared = field(&fields(&mut bridge, &space, "tasks", &task), "customer").cloned();
    assert!(matches!(cleared, None | Some(Value::Null)), "{cleared:?}");
}

#[test]
fn a_reference_that_is_not_space_and_id_is_refused() {
    let mut bridge = opened_with(LOCK);
    let target = customer(&mut bridge);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let (target_space, target_id) = (
        field(&target, "space").unwrap().clone(),
        field(&target, "id").unwrap().clone(),
    );
    for wrong in [
        // A bare object id, as a link takes it.
        target_id.clone(),
        map(&[("space", target_space.clone())]),
        map(&[("id", target_id.clone())]),
        map(&[("space", text("not hex")), ("id", target_id.clone())]),
        map(&[("space", target_space.clone()), ("id", text("not hex"))]),
        map(&[
            ("space", target_space.clone()),
            ("id", target_id.clone()),
            ("more", text("x")),
        ]),
    ] {
        let values = map(&[("customer", wrong.clone())]);
        let written = map(&[
            ("space", space.clone()),
            (
                "ops",
                Value::Array(vec![op("create", "tasks", None, values)]),
            ),
        ]);
        assert_eq!(
            code(&mut bridge, "write", written),
            "invalidValue",
            "{wrong:?}"
        );
    }
}

#[test]
fn a_link_takes_space_and_id_in_its_own_space_and_refuses_another() {
    // A field written nc.ref that the lock keeps as ncdb/link (RFC 0018,
    // "Existing link fields read as references"): the TypeScript SDK sends
    // { space, id }, and the link holds the object id.
    let mut bridge = opened_with(LOCK);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let project = write(
        &mut bridge,
        &space,
        vec![op("create", "projects", None, map(&[("name", text("p"))]))],
    )
    .remove(0);
    let own = map(&[("space", space.clone()), ("id", project.clone())]);
    let task = write(
        &mut bridge,
        &space,
        vec![op("create", "tasks", None, map(&[("project", own)]))],
    )
    .remove(0);
    assert_eq!(
        field(&fields(&mut bridge, &space, "tasks", &task), "project"),
        Some(&project)
    );

    let values = map(&[("project", customer(&mut bridge))]);
    let written = map(&[
        ("space", space.clone()),
        (
            "ops",
            Value::Array(vec![op("create", "tasks", None, values)]),
        ),
    ]);
    assert_eq!(code(&mut bridge, "write", written), "invalidValue");
}
