//! A day and a point in time through the bridge (RFC 0005, "Days and points
//! in time"; Carsten, 2026-11-21, NOC-434): the TypeScript SDK writes a day
//! as its `"YYYY-MM-DD"` text and a point in time as its milliseconds since
//! 1970-01-01T00:00:00.000Z, `null` clears either, and a read gives them
//! back in the same forms. Any other form, or a value outside the years 1
//! to 9999, is `invalidValue`.

use ncdb_core::cbor::Value;

use super::fields::{fields, op, opened_with, write};
use super::{Bridge, LOCK, code, field, map, ok, text};

/// 2026-11-21T19:08:44.636Z.
const NOW: u64 = 1_795_288_124_636;

/// The fields of a task created in a new space with `values`.
fn created(bridge: &mut Bridge, values: Value) -> (Value, Value) {
    let space = ok(bridge, "spaces.create", map(&[]));
    let task = write(bridge, &space, vec![op("create", "tasks", None, values)]).remove(0);
    (space, task)
}

#[test]
fn a_day_reads_back_as_its_text_and_a_point_in_time_as_its_milliseconds() {
    let mut bridge = opened_with(LOCK);
    let (space, task) = created(
        &mut bridge,
        map(&[
            ("doneOn", text("1969-12-31")),
            ("dueAt", Value::Unsigned(NOW)),
        ]),
    );
    let read = fields(&mut bridge, &space, "tasks", &task);
    assert_eq!(field(&read, "doneOn"), Some(&text("1969-12-31")));
    assert_eq!(field(&read, "dueAt"), Some(&Value::Unsigned(NOW)));

    // Before 1970, a point in time is negative; null clears a day.
    let update = |values| vec![op("update", "tasks", Some(&task), values)];
    write(
        &mut bridge,
        &space,
        update(map(&[
            ("doneOn", Value::Null),
            ("dueAt", Value::Negative(0)),
        ])),
    );
    let read = fields(&mut bridge, &space, "tasks", &task);
    let cleared = field(&read, "doneOn").cloned();
    assert!(matches!(cleared, None | Some(Value::Null)), "{cleared:?}");
    assert_eq!(field(&read, "dueAt"), Some(&Value::Negative(0)));
}

#[test]
fn another_form_or_a_value_outside_the_range_is_refused() {
    let mut bridge = opened_with(LOCK);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    for (name, wrong) in [
        ("doneOn", text("2026-02-30")),
        ("doneOn", text("21.11.2026")),
        ("doneOn", text("10000-01-01")),
        ("doneOn", Value::Unsigned(20_778)),
        ("dueAt", text("2026-11-21T19:08:44.636Z")),
        ("dueAt", Value::Float(1.5)),
        ("dueAt", Value::Unsigned(253_402_300_800_000)),
        ("dueAt", Value::Negative(62_135_596_800_000)),
    ] {
        let values = map(&[(name, wrong.clone())]);
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
            "{name}: {wrong:?}"
        );
    }
}

#[test]
fn an_index_on_a_point_in_time_takes_its_milliseconds() {
    let mut bridge = opened_with(LOCK);
    let (_, _) = created(&mut bridge, map(&[("dueAt", Value::Unsigned(NOW))]));
    let found = ok(
        &mut bridge,
        "find",
        map(&[(
            "query",
            map(&[("collection", text("tasks")), ("index", text("byDueAt"))]),
        )]),
    );
    let items = match field(&found, "items") {
        Some(Value::Array(items)) => items.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(items.len(), 1);
    let fields = field(&items[0], "fields").unwrap();
    assert_eq!(field(fields, "dueAt"), Some(&Value::Unsigned(NOW)));
}
