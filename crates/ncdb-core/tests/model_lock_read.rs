//! Reading `model.lock.json` (RFC 0005, "The structure of model.lock.json")
//! into a model the fold can use.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{
    Direction, Element, Kind, ModelId, ModelLock, Person, read::LockError,
};

/// RFC 0005's example lock at model version 2, with its keys in canonical
/// order (the RFC lists `notesAboutAssignee` before `dependsOn`).
const EXAMPLE: &str = include_str!("model_lock/rfc-0005-v2.lock.json");

fn example() -> ModelLock {
    ModelLock::from_json(EXAMPLE).expect("the RFC example reads")
}

#[test]
fn reads_the_model_entry() {
    let lock = example();
    assert_eq!(lock.model.version, 2);
    assert_eq!(
        lock.model.id.to_string(),
        "6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a"
    );
    assert_eq!(
        lock.model.hash,
        "sha256:a59fed6588bc432d392c671b9ce7816babffc1fd7208187e4f2dd2c952e0531d"
    );
    assert!(lock.plugins.is_empty());
}

#[test]
fn tells_the_fold_which_collections_and_fields_the_model_knows() {
    let lock = example();
    assert!(lock.knows_collection("tasks"));
    assert!(lock.knows_collection("people"));
    assert!(!lock.knows_collection("milestones"));
    assert!(lock.knows_field("tasks", "title"));
    assert!(!lock.knows_field("tasks", "summary"));
    assert!(!lock.knows_field("people", "title"));
    // Computed fields and views are not stored fields: no change writes them.
    assert!(!lock.knows_field("tasks", "blocked"));
}

#[test]
fn reads_each_field_type_with_what_it_needs() {
    let lock = example();
    let kind = |field: &str| lock.field("tasks", field).unwrap().kind.clone();
    assert_eq!(kind("priority"), Kind::Int);
    assert_eq!(kind("estimate"), Kind::Float);
    assert_eq!(kind("archived"), Kind::Boolean);
    assert_eq!(kind("title"), Kind::String);
    assert_eq!(
        kind("status"),
        Kind::Choice(vec!["open".into(), "in_progress".into(), "done".into()])
    );
    assert_eq!(kind("tags"), Kind::Set(Element::String));
    assert_eq!(kind("assignee"), Kind::Link(vec!["people".into()]));
    assert_eq!(kind("dependsOn"), Kind::Links(vec!["tasks".into()]));

    let title = lock.field("tasks", "title").unwrap();
    assert_eq!((title.rule, title.since), (1, 1));
    assert_eq!(
        title.description.as_deref(),
        Some("One line; what has to be done.")
    );
    let notes = lock.field("tasks", "notesAboutAssignee").unwrap();
    assert_eq!(notes.personal, Some(Person::Link("assignee".into())));
    assert_eq!(
        lock.field("people", "email").unwrap().personal,
        Some(Person::Itself)
    );
}

#[test]
fn reads_indexes_computed_fields_and_views() {
    let lock = example();
    let tasks = lock.collection("tasks").unwrap();
    assert_eq!(tasks.since, 1);
    assert_eq!(tasks.computed["blocked"].since, 1);
    assert_eq!(tasks.views["ready"].since, 1);
    let index = &tasks.indexes["byStatusPriority"];
    assert_eq!(index.equal, 1);
    assert_eq!(index.fields[0].field, "status");
    assert_eq!(index.fields[1].direction, Direction::Asc);
    assert_eq!(
        tasks.indexes["byPriorityDesc"].fields[0].direction,
        Direction::Desc
    );
}

#[test]
fn reads_plugins_and_plugin_field_types() {
    let text = r#"{
      "collections": {
        "tasks": {
          "fields": {
            "votes": { "config": { "step": 1 }, "rule": 2, "since": 3, "type": "acme/counter" }
          },
          "since": 1
        }
      },
      "format": 1,
      "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 3 },
      "plugins": { "acme": { "crate": "acme-ncdb-counter", "npm": "@acme/ncdb-counter" } }
    }"#;
    let lock = ModelLock::from_json(text).unwrap();
    assert_eq!(lock.model.id, ModelId(core::array::from_fn(|i| i as u8)));
    assert_eq!(
        lock.plugins["acme"].crate_name.as_deref(),
        Some("acme-ncdb-counter")
    );
    assert_eq!(
        lock.plugins["acme"].npm.as_deref(),
        Some("@acme/ncdb-counter")
    );
    let votes = lock.field("tasks", "votes").unwrap();
    assert_eq!(votes.kind.type_id(), "acme/counter");
    assert_eq!(votes.rule, 2);
}

#[test]
fn reads_a_deprecated_field_and_collection() {
    let text = r#"{
      "collections": {
        "tasks": {
          "deprecated": 4,
          "deprecatedReason": "Use issues.",
          "fields": {
            "priority": { "deprecated": 3, "deprecatedReason": "Use rank.", "rule": 1, "since": 1, "type": "ncdb/int" }
          },
          "since": 1
        }
      },
      "format": 1,
      "model": { "hash": "sha256:00", "id": "6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a", "version": 4 },
      "plugins": {}
    }"#;
    let lock = ModelLock::from_json(text).unwrap();
    let collection = lock
        .collection("tasks")
        .unwrap()
        .deprecated
        .clone()
        .unwrap();
    assert_eq!(
        (collection.since, collection.reason.as_str()),
        (4, "Use issues.")
    );
    let deprecation = lock
        .field("tasks", "priority")
        .unwrap()
        .deprecated
        .clone()
        .unwrap();
    assert_eq!(
        (deprecation.since, deprecation.reason.as_str()),
        (3, "Use rank.")
    );
}

fn error(text: &str) -> LockError {
    ModelLock::from_json(text).expect_err("the lock is refused")
}

#[test]
fn refuses_a_format_it_does_not_know() {
    let text = EXAMPLE.replace("\"format\": 1", "\"format\": 2");
    let error = error(&text);
    assert_eq!(error.path, "format");
    assert!(error.message.contains("format 2"), "{error}");
}

#[test]
fn refuses_a_lock_that_is_not_well_formed_and_names_where() {
    assert_eq!(error("[]").path, "");
    assert_eq!(error("{ \"format\": 1 ").path, "");
    let since = EXAMPLE.replacen(
        "\"since\": 2,\n          \"type\": \"ncdb/string\"",
        "\"since\": \"2\",\n          \"type\": \"ncdb/string\"",
        1,
    );
    assert_eq!(error(&since).path, "collections.people.fields.email.since");
    let id = EXAMPLE.replace("6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a", "6F1C");
    assert_eq!(error(&id).path, "model.id");
    let set = EXAMPLE.replace(
        "\"of\": { \"type\": \"ncdb/string\" }",
        "\"of\": { \"type\": \"ncdb/link\" }",
    );
    assert_eq!(error(&set).path, "collections.tasks.fields.tags.of.type");
    let direction = EXAMPLE.replace("\"direction\": \"desc\"", "\"direction\": \"down\"");
    assert_eq!(
        error(&direction).path,
        "collections.tasks.indexes.byPriorityDesc.fields.0.direction"
    );
    let missing = EXAMPLE.replace("\"options\": [\"open\", \"in_progress\", \"done\"],", "");
    assert_eq!(
        error(&missing).path,
        "collections.tasks.fields.status.options"
    );
}
