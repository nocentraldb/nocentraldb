//! An index's `scope` in `model.lock.json` (RFC 0005, Amendment 3): written
//! as `"space"` for an index declared with `scope: "space"`, and left out
//! for an index that spans every space of the member (NOC-81).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Draft, ModelId, ModelLock, Scope, build};

const LOCK: &str = r#"{
  "collections": {
    "tasks": {
      "fields": {
        "rank": { "rule": 1, "since": 1, "type": "ncdb/int" }
      },
      "indexes": {
        "byRank": {
          "equal": 0,
          "fields": [{ "direction": "asc", "field": "rank" }],
          "since": 1
        },
        "inSpace": {
          "equal": 0,
          "fields": [{ "direction": "asc", "field": "rank" }],
          "scope": "space",
          "since": 1
        }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

fn draft(scope: &str) -> Draft {
    Draft::from_json(&format!(
        r#"{{ "collections": {{ "tasks": {{
          "fields": {{ "rank": {{ "rule": 1, "type": "ncdb/int" }} }},
          "indexes": {{ "inSpace": {{ "equal": 0, "fields": [{{ "direction": "asc", "field": "rank" }}]{scope} }} }} }} }}, "plugins": {{}} }}"#
    ))
    .expect("the draft reads")
}

fn id() -> ModelId {
    ModelId([7; 16])
}

#[test]
fn reads_the_scope_of_an_index_and_an_index_without_one_spans_every_space() {
    let lock = ModelLock::from_json(LOCK).unwrap();
    let tasks = lock.collection("tasks").unwrap();
    assert_eq!(tasks.indexes["inSpace"].scope, Scope::Space);
    assert_eq!(tasks.indexes["byRank"].scope, Scope::Member);
}

#[test]
fn writes_scope_only_for_an_index_in_one_space() {
    let lock = ModelLock::from_json(LOCK).unwrap();
    assert_eq!(
        lock.to_json(),
        serde_json::from_str::<serde_json::Value>(LOCK).unwrap()
    );
    let written = lock.to_canonical_json();
    assert_eq!(written.matches("\"scope\"").count(), 1, "{written}");
    // The model hash covers the scope: changing it changes the model.
    let mut spanning = lock.clone();
    let index = spanning
        .collections
        .get_mut("tasks")
        .unwrap()
        .indexes
        .get_mut("inSpace")
        .unwrap();
    index.scope = Scope::Member;
    assert_ne!(spanning.compute_hash(), lock.compute_hash());
}

#[test]
fn refuses_a_scope_other_than_space() {
    let text = LOCK.replace("\"scope\": \"space\"", "\"scope\": \"member\"");
    let error = ModelLock::from_json(&text).unwrap_err();
    assert_eq!(error.path, "collections.tasks.indexes.inSpace.scope");
}

#[test]
fn a_build_keeps_the_scope_and_a_changed_scope_takes_the_new_version() {
    let v1 = build(&draft(r#", "scope": "space""#), None, id).unwrap().lock;
    let index = &v1.collection("tasks").unwrap().indexes["inSpace"];
    assert_eq!((index.scope, index.since), (Scope::Space, 1));
    let v2 = build(&draft(""), Some(&v1), id).unwrap().lock;
    let index = &v2.collection("tasks").unwrap().indexes["inSpace"];
    assert_eq!((index.scope, index.since), (Scope::Member, 2));
}
