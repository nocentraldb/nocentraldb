//! `Query::equal` on every field type an index may fix (RFC 0005,
//! "Indexes": `nc.string`, `nc.int`, `nc.float`, `nc.boolean`,
//! `nc.oneOf`, `nc.link` and `nc.ref`), with each value as the Rust SDK
//! writes it: a link as its `ObjectId`, a reference as its `Reference`.
//! NOC-423 found the TypeScript SDK's link and reference queries empty;
//! this checks the Rust SDK for the same case.

mod support;

use nocentraldb::{Db, ObjectId, OpenOptions, Reference, Value};
use support::{LOCK, now};

/// The fields an index here fixes, one index `eq<field>` each.
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

#[test]
fn equal_finds_the_object_on_every_field_type_an_index_may_fix() {
    let lock = with_equality_indexes();
    let db = now(Db::open(OpenOptions::new(&lock).in_memory())).expect("the lock opens");
    let space = now(db.spaces().create()).unwrap();
    let ids = |written: nocentraldb::Written| written.ids;
    let projects = ids(now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("p"))])?;
        tx.create("projects", [("name", Value::from("q"))])?;
        Ok(())
    }))
    .unwrap());
    let (p, q) = (projects[0], projects[1]);
    let reference = |id: ObjectId| Reference::new(space.id(), id);
    let tasks = ids(now(space.write(|tx| {
        for (title, status, n, estimate, urgent, project) in [
            ("a", "open", 1, 2.0, true, p),
            ("b", "done", 2, 2.5, false, q),
        ] {
            tx.create(
                "tasks",
                [
                    ("title", Value::from(title)),
                    ("status", Value::from(status)),
                    ("priority", Value::from(n)),
                    ("estimate", Value::from(estimate)),
                    ("urgent", Value::from(urgent)),
                    ("project", Value::from(project)),
                    ("customer", Value::from(reference(project))),
                ],
            )?;
        }
        Ok(())
    }))
    .unwrap());
    let (a, b) = (vec![tasks[0]], vec![tasks[1]]);
    let cases = [
        ("title", Value::from("a"), &a),
        ("status", Value::from("done"), &b),
        ("priority", Value::from(1), &a),
        ("estimate", Value::from(2.0), &a),
        ("estimate", Value::from(2.5), &b),
        ("urgent", Value::from(false), &b),
        ("project", Value::from(p), &a),
        ("project", Value::from(q), &b),
        ("customer", Value::from(reference(p)), &a),
        ("customer", Value::from(reference(q)), &b),
    ];
    for (name, value, expected) in cases {
        let query = db
            .index("tasks", &format!("eq{name}"))
            .unwrap()
            .equal([value]);
        let page = now(db.find(&query)).unwrap();
        let found: Vec<ObjectId> = page.items.iter().map(|item| item.id()).collect();
        assert_eq!(&found, expected, "{name}");
    }
}
