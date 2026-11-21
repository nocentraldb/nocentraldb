//! `.equal` on every field type an index may fix (RFC 0005, "Indexes":
//! `nc.string`, `nc.int`, `nc.float`, `nc.boolean`, `nc.oneOf`, `nc.link`
//! and `nc.ref`), with each value as the TypeScript SDK sends it: a link as
//! the target's 32 hex digits, a reference as `{ space, id }`, a whole
//! float as a whole number. The bridge reads them by the field's type in
//! the lock, as a write does (NOC-353, NOC-423).

use ncdb_core::cbor::Value;

use super::fields::{op, opened_with, write};
use super::{Bridge, LOCK, code, field, map, ok, text};

/// The index on one field whose name it takes, with that field as its
/// equality prefix.
const FIELDS: [&str; 7] = [
    "title", "status", "priority", "estimate", "urgent", "project", "customer",
];

/// The test lock whose tasks have, instead of their own indexes, an index
/// `eq<field>` per field of `FIELDS` with that field as its equality
/// prefix. The core checks that a query runs through its index, so no
/// other index starts with the same column.
fn with_equality_indexes() -> String {
    let indexes: Vec<String> = FIELDS
        .iter()
        .map(|name| {
            format!(
                r#""eq{name}": {{ "equal": 1, "fields": [{{ "direction": "asc", "field": "{name}" }}], "since": 1 }}"#
            )
        })
        .collect();
    let start = LOCK.find(r#""byPriority": {"#).expect("the tasks' indexes");
    let end = start + LOCK[start..].find("\n      },").expect("their end");
    format!("{}{}{}", &LOCK[..start], indexes.join(",\n"), &LOCK[end..])
}

fn find(bridge: &mut Bridge, index: &str, value: Value) -> Value {
    let query = map(&[
        ("collection", text("tasks")),
        ("index", text(index)),
        ("equal", Value::Array(vec![value])),
    ]);
    ok(bridge, "find", map(&[("query", query)]))
}

/// The ids of a page's objects, in order.
fn ids(page: &Value) -> Vec<Value> {
    let Some(Value::Array(items)) = field(page, "items") else {
        panic!("{page:?}")
    };
    items
        .iter()
        .map(|item| field(item, "$id").expect("an id").clone())
        .collect()
}

#[test]
fn equal_finds_the_object_on_every_field_type_an_index_may_fix() {
    let mut bridge = opened_with(&with_equality_indexes());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let projects = write(
        &mut bridge,
        &space,
        vec![
            op("create", "projects", None, map(&[("name", text("p"))])),
            op("create", "projects", None, map(&[("name", text("q"))])),
        ],
    );
    let reference = |id: &Value| map(&[("space", space.clone()), ("id", id.clone())]);
    let task = |title: &str, status: &str, n: u64, estimate: f64, urgent: bool, p: usize| {
        map(&[
            ("title", text(title)),
            ("status", text(status)),
            ("priority", Value::Unsigned(n)),
            ("estimate", Value::Float(estimate)),
            ("urgent", Value::Bool(urgent)),
            ("project", projects[p].clone()),
            ("customer", reference(&projects[p])),
        ])
    };
    let tasks = write(
        &mut bridge,
        &space,
        vec![
            op("create", "tasks", None, task("a", "open", 1, 2.0, true, 0)),
            op("create", "tasks", None, task("b", "done", 2, 2.5, false, 1)),
        ],
    );
    let (a, b) = (vec![tasks[0].clone()], vec![tasks[1].clone()]);
    let cases = [
        ("title", text("a"), &a),
        ("status", text("done"), &b),
        ("priority", Value::Unsigned(1), &a),
        // TypeScript has one number type; CBOR writes 2.0 as the integer 2.
        ("estimate", Value::Unsigned(2), &a),
        ("estimate", Value::Float(2.5), &b),
        ("urgent", Value::Bool(false), &b),
        ("project", projects[0].clone(), &a),
        ("project", projects[1].clone(), &b),
        ("customer", reference(&projects[0]), &a),
        ("customer", reference(&projects[1]), &b),
    ];
    for (name, value, expected) in cases {
        let page = find(&mut bridge, &format!("eq{name}"), value.clone());
        assert_eq!(&ids(&page), expected, "{name} = {value:?}");
    }
}

#[test]
fn equal_refuses_a_link_or_a_reference_it_cannot_read() {
    let mut bridge = opened_with(&with_equality_indexes());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    for (index, wrong) in [
        ("eqproject", text("not hex")),
        ("eqcustomer", text("0123456789abcdef0123456789abcdef")),
        ("eqcustomer", map(&[("space", space.clone())])),
    ] {
        let query = map(&[
            ("collection", text("tasks")),
            ("index", text(index)),
            ("equal", Value::Array(vec![wrong.clone()])),
        ]);
        let args = map(&[("query", query)]);
        assert_eq!(code(&mut bridge, "find", args), "invalidValue", "{wrong:?}");
    }
}
