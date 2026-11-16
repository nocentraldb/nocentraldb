//! Field values the TypeScript SDK writes through the bridge, by the
//! field's type (RFC 0009, "Field operations per field type"; NOC-353): an
//! object id arrives as its 32 hex digits, a text, and the bridge writes
//! it as a link where the field is one, so that it reads back as a link
//! and a Rust device reads it as an `ObjectId`.

use ncdb_core::cbor::Value;

use super::{Bridge, LOCK, as_text, field, map, now, ok, text};
use crate::id::{ObjectId, SpaceId};

fn opened_with(lock: &str) -> Bridge {
    let mut bridge = Bridge::new();
    ok(
        &mut bridge,
        "open",
        map(&[("lock", text(lock)), ("inMemory", Value::Bool(true))]),
    );
    bridge
}

/// One write of `ops` into `space`; the ids it created.
fn write(bridge: &mut Bridge, space: &Value, ops: Vec<Value>) -> Vec<Value> {
    let written = ok(
        bridge,
        "write",
        map(&[("space", space.clone()), ("ops", Value::Array(ops))]),
    );
    match field(&written, "ids") {
        Some(Value::Array(ids)) => ids.clone(),
        _ => panic!("{written:?}"),
    }
}

fn op(op: &str, collection: &str, id: Option<&Value>, values: Value) -> Value {
    let mut entries = vec![
        ("op", text(op)),
        ("collection", text(collection)),
        ("values", values),
    ];
    if let Some(id) = id {
        entries.push(("id", id.clone()));
    }
    map(&entries)
}

/// The fields of the object `id` of `collection`, as `get` answers them.
fn fields(bridge: &mut Bridge, space: &Value, collection: &str, id: &Value) -> Value {
    let read = ok(
        bridge,
        "get",
        map(&[
            ("space", space.clone()),
            ("collection", text(collection)),
            ("id", id.clone()),
        ]),
    );
    field(&read, "fields").expect("its fields").clone()
}

/// The object `id` of `collection` as the Rust SDK reads it, on the
/// bridge's own database.
fn native(bridge: &Bridge, space: &Value, collection: &str, id: &Value) -> crate::Object {
    let db = bridge.db.as_ref().expect("a database is open");
    let space = now(db.spaces().get(as_text(space).parse::<SpaceId>().unwrap())).unwrap();
    let id: ObjectId = as_text(id).parse().unwrap();
    now(space.get(collection, id)).unwrap().expect("the object")
}

#[test]
fn a_link_written_as_an_object_id_reads_back_as_that_link() {
    let mut bridge = opened_with(LOCK);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let projects = write(
        &mut bridge,
        &space,
        vec![
            op("create", "projects", None, map(&[("name", text("a"))])),
            op("create", "projects", None, map(&[("name", text("b"))])),
        ],
    );
    let task = write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "tasks",
            None,
            map(&[("project", projects[0].clone())]),
        )],
    )
    .remove(0);
    assert_eq!(
        field(&fields(&mut bridge, &space, "tasks", &task), "project"),
        Some(&projects[0])
    );
    let linked: ObjectId = native(&bridge, &space, "tasks", &task)
        .get::<ObjectId>("project")
        .unwrap()
        .expect("a link");
    assert_eq!(linked.to_string(), as_text(&projects[0]));

    // An update moves the link; null clears it.
    write(
        &mut bridge,
        &space,
        vec![op(
            "update",
            "tasks",
            Some(&task),
            map(&[("project", projects[1].clone())]),
        )],
    );
    assert_eq!(
        field(&fields(&mut bridge, &space, "tasks", &task), "project"),
        Some(&projects[1])
    );
    write(
        &mut bridge,
        &space,
        vec![op(
            "update",
            "tasks",
            Some(&task),
            map(&[("project", Value::Null)]),
        )],
    );
    let cleared = field(&fields(&mut bridge, &space, "tasks", &task), "project").cloned();
    assert!(matches!(cleared, None | Some(Value::Null)), "{cleared:?}");
}

#[test]
fn a_link_that_is_no_object_id_is_refused() {
    let mut bridge = opened_with(LOCK);
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let ops = Value::Array(vec![op(
        "create",
        "tasks",
        None,
        map(&[("project", text("not an id"))]),
    )]);
    assert_eq!(
        super::code(&mut bridge, "write", map(&[("space", space), ("ops", ops)])),
        "invalidValue"
    );
}

/// The test lock with `tasks.related`, many links to tasks.
fn with_many_links() -> String {
    let lock = LOCK.replacen(
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },"#,
        r#""estimate": { "rule": 1, "since": 1, "type": "ncdb/float" },
        "related": { "rule": 1, "since": 1, "targets": ["tasks"], "type": "ncdb/links" },"#,
        1,
    );
    assert!(lock.contains("ncdb/links"));
    lock
}

#[test]
fn many_links_take_object_ids_to_add_and_remove() {
    let mut bridge = opened_with(&with_many_links());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let targets = write(
        &mut bridge,
        &space,
        vec![
            op("create", "tasks", None, map(&[("title", text("a"))])),
            op("create", "tasks", None, map(&[("title", text("b"))])),
        ],
    );
    let task = write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "tasks",
            None,
            map(&[("related", map(&[("$add", Value::Array(targets.clone()))]))]),
        )],
    )
    .remove(0);
    let related = |bridge: &mut Bridge| {
        let mut ids: Vec<String> = match field(&fields(bridge, &space, "tasks", &task), "related") {
            Some(Value::Array(ids)) => ids.iter().map(|id| as_text(id).to_owned()).collect(),
            other => panic!("{other:?}"),
        };
        ids.sort_unstable();
        ids
    };
    let mut both: Vec<String> = targets.iter().map(|id| as_text(id).to_owned()).collect();
    both.sort_unstable();
    assert_eq!(related(&mut bridge), both);
    let on_rust = native(&bridge, &space, "tasks", &task);
    let elements = on_rust
        .get::<&[crate::Value]>("related")
        .unwrap()
        .expect("links");
    assert!(
        elements
            .iter()
            .all(|element| ObjectId::try_from(element).is_ok())
    );

    write(
        &mut bridge,
        &space,
        vec![op(
            "update",
            "tasks",
            Some(&task),
            map(&[(
                "related",
                map(&[("$remove", Value::Array(vec![targets[0].clone()]))]),
            )]),
        )],
    );
    assert_eq!(related(&mut bridge), [as_text(&targets[1]).to_owned()]);
}

#[test]
fn a_personal_field_is_sealed_under_the_person_its_link_names_in_the_same_write() {
    // `tasks.title` is personal to the task's `project`, and
    // `projects.name` to the project itself.
    let mut bridge = opened_with(&crate::personal_write_tests::lock());
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let project = write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "projects",
            None,
            map(&[("name", text("Anna"))]),
        )],
    )
    .remove(0);
    let task = write(
        &mut bridge,
        &space,
        vec![op(
            "create",
            "tasks",
            None,
            map(&[("project", project.clone()), ("title", text("about Anna"))]),
        )],
    )
    .remove(0);
    let read = fields(&mut bridge, &space, "tasks", &task);
    assert_eq!(field(&read, "title"), Some(&text("about Anna")));
    assert_eq!(field(&read, "project"), Some(&project));
}
