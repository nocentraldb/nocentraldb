//! `ncdb model build`'s core: from the description of a schema and the
//! previous lock to the next lock (RFC 0005, "The structure of
//! model.lock.json" and "The additive-change rule").

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Draft, ModelId, ModelLock, Refusal, build};

const EXAMPLE: &str = include_str!("model_lock/rfc-0005-v2.lock.json");

/// The task board at model version 1, as RFC 0005 describes it (without
/// the rich-text body).
const V1: &str = r#"{
  "collections": {
    "tasks": {
      "fields": {
        "title": { "description": "One line; what has to be done.", "rule": 1, "type": "ncdb/string" },
        "status": { "options": ["open", "in_progress", "done"], "rule": 1, "type": "ncdb/choice" },
        "priority": { "rule": 1, "type": "ncdb/int" },
        "dependsOn": { "rule": 1, "targets": ["tasks"], "type": "ncdb/links" }
      },
      "computed": ["blocked"],
      "views": ["ready"],
      "indexes": {
        "byStatusPriority": { "equal": 1, "fields": [{ "direction": "asc", "field": "status" }, { "direction": "asc", "field": "priority" }] }
      }
    }
  },
  "plugins": {}
}"#;

/// The task board at model version 2, as RFC 0005's schema declares it.
const V2: &str = r#"{
  "collections": {
    "people": {
      "fields": {
        "name": { "personal": { "person": "self" }, "rule": 1, "type": "ncdb/string" },
        "email": { "personal": { "person": "self" }, "rule": 1, "type": "ncdb/string" }
      }
    },
    "tasks": {
      "fields": {
        "title": { "description": "One line; what has to be done.", "rule": 1, "type": "ncdb/string" },
        "status": { "options": ["open", "in_progress", "done"], "rule": 1, "type": "ncdb/choice" },
        "priority": { "rule": 1, "type": "ncdb/int" },
        "estimate": { "rule": 1, "type": "ncdb/float" },
        "archived": { "rule": 1, "type": "ncdb/boolean" },
        "tags": { "of": { "type": "ncdb/string" }, "rule": 1, "type": "ncdb/set" },
        "assignee": { "rule": 1, "targets": ["people"], "type": "ncdb/link" },
        "notesAboutAssignee": { "personal": { "person": "assignee" }, "rule": 1, "type": "ncdb/string" },
        "dependsOn": { "rule": 1, "targets": ["tasks"], "type": "ncdb/links" }
      },
      "computed": ["blocked"],
      "views": ["ready"],
      "indexes": {
        "byStatusPriority": { "equal": 1, "fields": [{ "direction": "asc", "field": "status" }, { "direction": "asc", "field": "priority" }] },
        "byPriorityDesc": { "equal": 0, "fields": [{ "direction": "desc", "field": "priority" }] }
      }
    }
  },
  "plugins": {}
}"#;

const ID: ModelId = ModelId([
    0x6f, 0x1c, 0x0e, 0x2a, 0x9b, 0x7d, 0x4c, 0x3e, 0x8a, 0x5f, 0x0b, 0x1d, 0x2c, 0x3e, 0x4f, 0x5a,
]);

fn draft(text: &str) -> Draft {
    Draft::from_json(text).expect("the draft reads")
}

fn first() -> ModelLock {
    build(&draft(V1), None, || ID)
        .expect("version 1 builds")
        .lock
}

fn no_new_id() -> ModelId {
    panic!("a build with a previous lock keeps its model.id")
}

#[test]
fn the_first_build_draws_a_model_id_and_is_version_1() {
    let lock = first();
    assert_eq!((lock.model.id, lock.model.version), (ID, 1));
    assert_eq!(lock.model.hash, lock.compute_hash());
    let tasks = lock.collection("tasks").unwrap();
    assert!(tasks.fields.values().all(|f| f.since == 1));
    assert_eq!(
        (
            tasks.since,
            tasks.computed["blocked"].since,
            tasks.indexes["byStatusPriority"].since
        ),
        (1, 1, 1)
    );
    let random = build(&draft(V1), None, ModelId::random).unwrap().lock;
    assert_ne!(random.model.id, ID);
}

#[test]
fn building_version_2_on_version_1_gives_the_rfc_example_byte_for_byte() {
    let built = build(&draft(V2), Some(&first()), no_new_id).expect("version 2 is additive");
    assert_eq!(built.lock.to_canonical_json(), EXAMPLE);
}

#[test]
fn an_unchanged_schema_keeps_the_lock() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let built = build(&draft(V2), Some(&v2), no_new_id).unwrap();
    assert_eq!(built.lock, v2);
}

#[test]
fn a_change_of_descriptions_only_keeps_the_model_version() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2
        .replace("One line; what has to be done.", "What has to be done.")
        .replace(
            r#""priority": { "rule""#,
            r#""priority": { "description": "Higher first.", "rule""#,
        );
    let lock = build(&draft(&text), Some(&v2), no_new_id).unwrap().lock;
    assert_eq!(lock.model.version, 2);
    assert_eq!(lock.model.hash, v2.model.hash);
    assert_eq!(
        lock.field("tasks", "title").unwrap().description.as_deref(),
        Some("What has to be done.")
    );
    assert_eq!(
        lock.field("tasks", "priority")
            .unwrap()
            .description
            .as_deref(),
        Some("Higher first.")
    );
}

#[test]
fn a_deprecation_records_the_version_that_made_it_and_keeps_it() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2.replace(
        r#""priority": { "rule""#,
        r#""priority": { "deprecated": "Use rank.", "rule""#,
    );
    let v3 = build(&draft(&text), Some(&v2), no_new_id).unwrap().lock;
    assert_eq!(v3.model.version, 3);
    let deprecation = v3
        .field("tasks", "priority")
        .unwrap()
        .deprecated
        .clone()
        .unwrap();
    assert_eq!(
        (deprecation.since, deprecation.reason.as_str()),
        (3, "Use rank.")
    );
    let text = text.replace(
        r#""estimate": {"#,
        r#""rank": { "rule": 1, "type": "ncdb/int" }, "estimate": {"#,
    );
    let v4 = build(&draft(&text), Some(&v3), no_new_id).unwrap().lock;
    assert_eq!(v4.model.version, 4);
    assert_eq!(
        v4.field("tasks", "priority")
            .unwrap()
            .deprecated
            .as_ref()
            .unwrap()
            .since,
        3
    );
    assert_eq!(v4.field("tasks", "rank").unwrap().since, 4);
}

#[test]
fn a_changed_index_takes_the_new_version_and_an_unchanged_one_keeps_its_own() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2.replace(
        r#""equal": 0, "fields": [{ "direction": "desc""#,
        r#""equal": 1, "fields": [{ "direction": "desc""#,
    );
    let v3 = build(&draft(&text), Some(&v2), no_new_id).unwrap().lock;
    let tasks = v3.collection("tasks").unwrap();
    assert_eq!(
        (
            tasks.indexes["byPriorityDesc"].since,
            tasks.indexes["byStatusPriority"].since
        ),
        (3, 1)
    );
}

#[test]
fn a_collection_deprecation_records_the_version_that_made_it_and_keeps_it() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2.replace(
        r#""people": {"#,
        r#""people": { "deprecated": "Use members.","#,
    );
    let v3 = build(&draft(&text), Some(&v2), no_new_id).unwrap().lock;
    assert_eq!(v3.model.version, 3);
    let deprecation = v3.collection("people").unwrap().deprecated.clone().unwrap();
    assert_eq!(
        (deprecation.since, deprecation.reason.as_str()),
        (3, "Use members.")
    );
    let text = text.replace(
        r#""estimate": {"#,
        r#""rank": { "rule": 1, "type": "ncdb/int" }, "estimate": {"#,
    );
    let v4 = build(&draft(&text), Some(&v3), no_new_id).unwrap().lock;
    assert_eq!(v4.model.version, 4);
    assert_eq!(
        v4.collection("people")
            .unwrap()
            .deprecated
            .as_ref()
            .unwrap()
            .since,
        3
    );
}

#[test]
fn a_collection_the_schema_no_longer_deprecates_is_refused() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2.replace(
        r#""people": {"#,
        r#""people": { "deprecated": "Use members.","#,
    );
    let v3 = build(&draft(&text), Some(&v2), no_new_id).unwrap().lock;
    let refusals = build(&draft(V2), Some(&v3), no_new_id).unwrap_err();
    assert_eq!(
        refusals,
        vec![Refusal::CollectionUndeprecated {
            collection: "people".into()
        }]
    );
}

#[test]
fn a_change_that_is_not_additive_is_refused_and_builds_nothing() {
    let v2 = ModelLock::from_json(EXAMPLE).unwrap();
    let text = V2.replace(
        r#""priority": { "rule": 1, "type": "ncdb/int" }"#,
        r#""priority": { "options": ["low", "high"], "rule": 1, "type": "ncdb/choice" }"#,
    );
    let refusals = build(&draft(&text), Some(&v2), no_new_id).unwrap_err();
    assert_eq!(
        refusals,
        vec![Refusal::TypeChanged {
            collection: "tasks".into(),
            field: "priority".into(),
            from: "ncdb/int".into(),
            to: "ncdb/choice".into()
        }]
    );
}

#[test]
fn the_first_build_refuses_a_model_that_breaks_the_rules() {
    let text = V1.replace(r#""priority": {"#, r#""Priority": {"#);
    let refusals = build(&draft(&text), None, || ID).unwrap_err();
    assert!(
        matches!(&refusals[0], Refusal::BadName { name, .. } if name == "Priority"),
        "{refusals:?}"
    );
}

#[test]
fn a_draft_that_is_not_well_formed_is_refused_with_its_path() {
    let error =
        Draft::from_json(&V1.replace(r#""views": ["ready"]"#, r#""views": "ready""#)).unwrap_err();
    assert_eq!(error.path, "collections.tasks.views");
    let error =
        Draft::from_json(&V1.replace(r#""priority": { "rule": 1"#, r#""priority": { "rule": "1""#))
            .unwrap_err();
    assert_eq!(error.path, "collections.tasks.fields.priority.rule");
    let error = Draft::from_json(&V1.replace(r#""equal": 1"#, r#""equal": -1"#)).unwrap_err();
    assert_eq!(
        error.path,
        "collections.tasks.indexes.byStatusPriority.equal"
    );
}

/// V1 with a plugin field `votes` of `acme/counter` with this `config`.
fn with_votes(config: &str) -> String {
    V1.replace(
        r#""priority": { "rule": 1, "type": "ncdb/int" },"#,
        &format!(
            r#""priority": {{ "rule": 1, "type": "ncdb/int" }},
        "votes": {{ "config": {config}, "rule": 1, "type": "acme/counter" }},"#
        ),
    )
    .replace(
        r#""plugins": {}"#,
        r#""plugins": { "acme": { "npm": "@acme/x" } }"#,
    )
}

/// RFC 0005, "Refused", and RFC 0011, "A type that is not registered"
/// (NOC-251): a build that changes a plugin field's `config` is refused and
/// builds nothing.
#[test]
fn a_changed_plugin_config_is_refused_and_builds_nothing() {
    let previous = build(&draft(&with_votes(r#"{ "step": 1 }"#)), None, || ID)
        .expect("the first version builds")
        .lock;

    let refusals = build(
        &draft(&with_votes(r#"{ "step": 2 }"#)),
        Some(&previous),
        no_new_id,
    )
    .unwrap_err();

    assert_eq!(
        refusals,
        vec![Refusal::ConfigChanged {
            collection: "tasks".into(),
            field: "votes".into(),
            type_id: "acme/counter".into(),
        }]
    );
}
