//! `ncdb/richText` in `model.lock.json` (RFC 0013, "The field in the schema
//! and the lock"): a built-in type at rule 1, without configuration, which
//! the lock reads and writes back, which cannot be personal or indexed, and
//! which a field of another type cannot become. A device opens a lock with
//! it; until the core implements rule 1, its operations park as "unknown
//! type", as a plugin type without code does (RFC 0011).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Draft, Kind, ModelId, ModelLock, Refusal, build, check};
use ncdb_core::view::Model;

/// Notes with a title and a rich-text body.
const LOCK: &str = r#"{
  "collections": {
    "notes": {
      "fields": {
        "body": { "rule": 1, "since": 1, "type": "ncdb/richText" },
        "title": { "rule": 1, "since": 1, "type": "ncdb/string" }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

const BODY: &str = r#""body": { "rule": 1, "since": 1, "type": "ncdb/richText" }"#;

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).expect("the lock reads")
}

/// The lock with `body` written as `entry`, at `version`, with its hash.
fn body_as(version: u64, entry: &str) -> ModelLock {
    let mut lock = ModelLock::from_json(&LOCK.replace(BODY, &format!(r#""body": {entry}"#)))
        .expect("the lock reads");
    lock.model.version = version;
    lock.model.hash = lock.compute_hash();
    lock
}

#[test]
fn reads_a_rich_text_field() {
    let lock = lock();
    let body = lock.field("notes", "body").unwrap();
    assert_eq!(body.kind, Kind::RichText);
    assert_eq!(body.kind.type_id(), "ncdb/richText");
    assert_eq!(body.kind.describe(), "ncdb/richText");
    assert_eq!(body.rule, 1);
}

#[test]
fn writes_it_back_as_it_was_read() {
    assert_eq!(
        lock().to_json(),
        serde_json::from_str::<serde_json::Value>(LOCK).unwrap()
    );
}

#[test]
fn is_not_a_basic_value_and_cannot_be_indexed() {
    assert!(!Kind::RichText.is_basic());
    assert!(!Kind::RichText.is_indexable());
    assert_eq!(Kind::RichText.options(), None);
    assert_eq!(Kind::RichText.targets(), None);
    assert_eq!(check::validate(&lock()), vec![]);
}

#[test]
fn a_rich_text_field_cannot_be_personal() {
    let lock = body_as(
        1,
        r#"{ "personal": { "person": "self" }, "rule": 1, "since": 1, "type": "ncdb/richText" }"#,
    );
    let refusals = check::validate(&lock);
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            Refusal::PersonalNotBasic { field, type_id, .. }
                if field == "body" && type_id == "ncdb/richText"
        )),
        "{refusals:?}"
    );
}

#[test]
fn an_index_cannot_use_a_rich_text_field() {
    let text = LOCK.replace(
        r#""since": 1
    }
  },"#,
        r#""indexes": { "byBody": { "equal": 0, "fields": [{ "direction": "asc", "field": "body" }], "since": 1 } },
      "since": 1
    }
  },"#,
    );
    let refusals = check::validate(&ModelLock::from_json(&text).unwrap());
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            Refusal::IndexNotIndexable { field, type_id, .. }
                if field == "body" && type_id == "ncdb/richText"
        )),
        "{refusals:?}"
    );
}

#[test]
fn only_rule_1_exists() {
    let lock = body_as(1, r#"{ "rule": 2, "since": 1, "type": "ncdb/richText" }"#);
    let refusals = check::validate(&lock);
    assert!(
        refusals.iter().any(|r| matches!(
            r,
            Refusal::UnknownRule { field, rule: 2, .. } if field == "body"
        )),
        "{refusals:?}"
    );
}

#[test]
fn a_text_field_cannot_become_rich_text_nor_rich_text_a_text_field() {
    let text = body_as(1, r#"{ "rule": 1, "since": 1, "type": "ncdb/string" }"#);
    let rich = body_as(2, r#"{ "rule": 1, "since": 1, "type": "ncdb/richText" }"#);
    let refusals = check::successor(&text, &rich);
    assert!(
        refusals
            .iter()
            .any(|r| matches!(r, Refusal::TypeChanged { field, .. } if field == "body")),
        "{refusals:?}"
    );
    let back = body_as(3, r#"{ "rule": 1, "since": 1, "type": "ncdb/string" }"#);
    assert!(!check::successor(&rich, &back).is_empty());
}

/// The schema's description of the notes, with `fields` as its fields.
fn draft(fields: &str) -> Draft {
    Draft::from_json(&format!(
        r#"{{ "collections": {{ "notes": {{ "fields": {{ {fields} }}, "computed": [], "views": [], "indexes": {{}} }} }}, "plugins": {{}} }}"#
    ))
    .expect("the description reads")
}

const ID: ModelId = ModelId([7; 16]);

#[test]
fn model_build_adds_a_rich_text_field_with_the_version_that_added_it() {
    let title = r#""title": { "rule": 1, "type": "ncdb/string" }"#;
    let first = build(&draft(title), None, || ID).unwrap().lock;
    let second = build(
        &draft(&format!(
            r#"{title}, "body": {{ "rule": 1, "type": "ncdb/richText" }}"#
        )),
        Some(&first),
        || unreachable!("the model has its id"),
    )
    .expect("adding a rich-text field is additive")
    .lock;
    assert_eq!(second.model.version, 2);
    assert_eq!(
        second.to_json()["collections"]["notes"]["fields"]["body"],
        serde_json::json!({ "rule": 1, "since": 2, "type": "ncdb/richText" })
    );
}

#[test]
fn a_device_opens_the_lock_and_parks_the_field_until_the_core_folds_it() {
    let model = Model::from_lock(&lock()).expect("the device opens the lock");
    assert!(model.field("notes", "title").is_some());
    assert!(model.field("notes", "body").is_none());
    assert_eq!(
        model.unregistered_type("notes", "body"),
        Some(("ncdb/richText", 1))
    );
}
