//! Ad-hoc spaces through the bridge (NOC-318 S4; RFC 0009, "Sub-agents and
//! ad-hoc spaces"; the TypeScript shapes the owner decided on NOC-351,
//! Carsten, 2026-11-16): `spaces.createAdHoc`, `space.sourceOf` and
//! `space.copyOf`.

use ncdb_core::cbor::Value;

use super::{Bridge, as_text, code, field, get_task, map, now, ok, opened, text, write_one};
use crate::value::Value as Field;

/// Creates a task in `space` with `values` and answers its id.
fn task(bridge: &mut Bridge, space: &Value, values: Value) -> Value {
    let written = write_one(
        bridge,
        space,
        map(&[
            ("op", text("create")),
            ("collection", text("tasks")),
            ("values", values),
        ]),
    );
    let Some(Value::Array(ids)) = field(&written, "ids") else {
        panic!("{written:?}")
    };
    ids[0].clone()
}

/// A source as `spaces.createAdHoc` takes it.
fn source(space: &Value, collection: &str, id: &Value) -> Value {
    map(&[
        ("space", space.clone()),
        ("collection", text(collection)),
        ("id", id.clone()),
    ])
}

fn create_ad_hoc(copy: Vec<Value>) -> Value {
    map(&[("copy", Value::Array(copy))])
}

fn copies(created: &Value) -> &[Value] {
    match field(created, "copies") {
        Some(Value::Array(copies)) => copies,
        other => panic!("no copies: {other:?}"),
    }
}

#[test]
fn create_ad_hoc_answers_the_space_its_notice_and_each_source_with_its_copy() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let a = task(&mut bridge, &space, map(&[("title", text("A"))]));
    let b = task(&mut bridge, &space, map(&[("title", text("B"))]));

    let created = ok(
        &mut bridge,
        "spaces.createAdHoc",
        create_ad_hoc(vec![
            source(&space, "tasks", &a),
            source(&space, "tasks", &b),
        ]),
    );

    let ad_hoc = field(&created, "space").expect("the space").clone();
    assert_eq!(as_text(&ad_hoc).len(), 32);
    assert_ne!(ad_hoc, space);
    let notice = field(&created, "notice").expect("a notice");
    assert_eq!(field(notice, "code"), Some(&text("adHocSpaceCreated")));
    assert!(!as_text(field(notice, "text").expect("its text")).is_empty());
    assert_eq!(field(&created, "clearedLinks"), Some(&Value::Unsigned(0)));
    assert_eq!(
        field(&created, "pluginFieldsNotCopied"),
        Some(&Value::Unsigned(0))
    );
    let [first, second] = copies(&created) else {
        panic!("two copies: {created:?}")
    };
    assert_eq!(field(first, "source"), Some(&a));
    assert_eq!(field(second, "source"), Some(&b));
    let copy = field(first, "copy").expect("a copy").clone();
    assert_ne!(copy, a, "a copy gets a fresh id");

    let read = get_task(&mut bridge, &ad_hoc, as_text(&copy));
    let fields = field(&read, "fields").expect("its fields");
    assert_eq!(field(fields, "title"), Some(&text("A")));
}

#[test]
fn a_link_whose_target_was_not_copied_is_counted_as_cleared() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    // The bridge writes no link yet, so the Rust SDK writes this one.
    let db = bridge.db.as_ref().expect("open");
    let id = as_text(&space).parse().expect("a space id");
    let in_rust = now(db.spaces().get(id)).expect("the space");
    let written = now(in_rust.write(|tx| {
        let project = tx.create("projects", [("name", Field::from("P"))])?;
        tx.create(
            "tasks",
            [
                ("title", Field::from("A")),
                ("project", Field::from(project)),
            ],
        )?;
        Ok(())
    }))
    .expect("written");
    let linked = text(&written.ids[1].to_string());

    let created = ok(
        &mut bridge,
        "spaces.createAdHoc",
        create_ad_hoc(vec![source(&space, "tasks", &linked)]),
    );
    assert_eq!(field(&created, "clearedLinks"), Some(&Value::Unsigned(1)));
}

#[test]
fn the_lookups_find_a_copys_source_and_a_sources_copy() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let a = task(&mut bridge, &space, map(&[("title", text("A"))]));
    let created = ok(
        &mut bridge,
        "spaces.createAdHoc",
        create_ad_hoc(vec![source(&space, "tasks", &a)]),
    );
    let ad_hoc = field(&created, "space").expect("the space").clone();
    let copy = field(&copies(&created)[0], "copy").expect("a copy").clone();

    let found = ok(
        &mut bridge,
        "space.sourceOf",
        map(&[("space", ad_hoc.clone()), ("copy", copy.clone())]),
    );
    assert_eq!(field(&found, "space"), Some(&space));
    assert_eq!(field(&found, "id"), Some(&a));

    let found = ok(
        &mut bridge,
        "space.copyOf",
        map(&[
            ("space", ad_hoc.clone()),
            ("sourceSpace", space.clone()),
            ("source", a.clone()),
        ]),
    );
    assert_eq!(found, copy);
}

#[test]
fn the_lookups_answer_null_for_what_this_database_did_not_copy() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let a = task(&mut bridge, &space, map(&[("title", text("A"))]));

    let none = ok(
        &mut bridge,
        "space.sourceOf",
        map(&[("space", space.clone()), ("copy", a.clone())]),
    );
    assert_eq!(none, Value::Null);
    let none = ok(
        &mut bridge,
        "space.copyOf",
        map(&[
            ("space", space.clone()),
            ("sourceSpace", space.clone()),
            ("source", a),
        ]),
    );
    assert_eq!(none, Value::Null);
}

#[test]
fn create_ad_hoc_refuses_what_the_rust_sdk_refuses() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let a = task(&mut bridge, &space, map(&[("title", text("A"))]));
    let unknown = text("0123456789abcdef0123456789abcdef");

    let not_held = create_ad_hoc(vec![source(&space, "tasks", &unknown)]);
    assert_eq!(
        code(&mut bridge, "spaces.createAdHoc", not_held),
        "notFound"
    );

    let twice = create_ad_hoc(vec![
        source(&space, "tasks", &a),
        source(&space, "tasks", &a),
    ]);
    assert_eq!(
        code(&mut bridge, "spaces.createAdHoc", twice),
        "invalidValue"
    );

    for option in ["personal", "local"] {
        let args = map(&[
            ("copy", Value::Array(vec![source(&space, "tasks", &a)])),
            (option, Value::Bool(true)),
        ]);
        assert_eq!(
            code(&mut bridge, "spaces.createAdHoc", args),
            "notAllowed",
            "{option}"
        );
    }

    let not_a_list = map(&[("copy", text("tasks"))]);
    assert_eq!(
        code(&mut bridge, "spaces.createAdHoc", not_a_list),
        "invalidValue"
    );
}
