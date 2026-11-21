//! `ncdb/date` and `ncdb/timestamp` in `model.lock.json` (RFC 0005, "Days
//! and points in time"): two basic values, which the lock reads and writes
//! back, which can be personal and indexed, and which are no set elements
//! in v1. A text field cannot become one: that changes its type.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Kind, ModelLock, Refusal, check};

/// People with a personal birthday, and events with a point in time and a
/// day, indexed by the point in time.
const LOCK: &str = r#"{
  "collections": {
    "events": {
      "fields": {
        "doneOn": { "rule": 1, "since": 1, "type": "ncdb/date" },
        "occurredAt": { "rule": 1, "since": 1, "type": "ncdb/timestamp" }
      },
      "indexes": {
        "byOccurredAt": {
          "equal": 0,
          "fields": [{ "direction": "desc", "field": "occurredAt" }, { "direction": "asc", "field": "doneOn" }],
          "since": 1
        }
      },
      "since": 1
    },
    "people": {
      "fields": {
        "birthday": { "personal": { "person": "self" }, "rule": 1, "since": 1, "type": "ncdb/date" }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).expect("the lock reads")
}

#[test]
fn reads_a_day_and_a_point_in_time() {
    let lock = lock();
    let done_on = lock.field("events", "doneOn").unwrap();
    assert_eq!(done_on.kind, Kind::Date);
    assert_eq!(done_on.kind.type_id(), "ncdb/date");
    let occurred_at = lock.field("events", "occurredAt").unwrap();
    assert_eq!(occurred_at.kind, Kind::Timestamp);
    assert_eq!(occurred_at.kind.type_id(), "ncdb/timestamp");
}

#[test]
fn writes_both_back_as_they_were_read() {
    assert_eq!(
        lock().to_json(),
        serde_json::from_str::<serde_json::Value>(LOCK).unwrap()
    );
}

#[test]
fn both_are_basic_values_that_can_be_personal_and_indexed() {
    for kind in [Kind::Date, Kind::Timestamp] {
        assert!(kind.is_basic(), "{kind:?}");
        assert!(kind.is_indexable(), "{kind:?}");
    }
    assert_eq!(check::validate(&lock()), vec![]);
}

#[test]
fn neither_is_a_set_element_in_v1() {
    for type_id in ["ncdb/date", "ncdb/timestamp"] {
        let text = LOCK.replace(
            r#""doneOn": { "rule": 1, "since": 1, "type": "ncdb/date" }"#,
            &format!(
                r#""doneOn": {{ "of": {{ "type": "{type_id}" }}, "rule": 1, "since": 1, "type": "ncdb/set" }}"#
            ),
        );
        let error = ModelLock::from_json(&text).unwrap_err();
        assert_eq!(error.path, "collections.events.fields.doneOn.of.type");
    }
}

/// The lock at `version` with `events.doneOn` of `type_id`.
fn done_on_as(version: u64, type_id: &str) -> ModelLock {
    let text = LOCK.replace(
        r#""doneOn": { "rule": 1, "since": 1, "type": "ncdb/date" }"#,
        &format!(r#""doneOn": {{ "rule": 1, "since": 1, "type": "{type_id}" }}"#),
    );
    let mut lock = ModelLock::from_json(&text).unwrap();
    lock.model.version = version;
    lock.model.hash = lock.compute_hash();
    lock
}

#[test]
fn a_text_field_that_holds_dates_cannot_become_a_day() {
    let text = done_on_as(1, "ncdb/string");
    let day = done_on_as(2, "ncdb/date");
    let refusals = check::successor(&text, &day);
    assert!(
        refusals
            .iter()
            .any(|r| matches!(r, Refusal::TypeChanged { field, .. } if field == "doneOn")),
        "{refusals:?}"
    );
    // Nor can a day become a point in time.
    let point = done_on_as(3, "ncdb/timestamp");
    assert!(!check::successor(&day, &point).is_empty());
}
