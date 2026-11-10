//! `ncdb/ref` in `model.lock.json` (RFC 0005, Amendment 5): a reference
//! field carries `targets`, the collections a target may be in, as a link
//! does. The lock reads and writes it, the checks hold its targets to
//! collections of the model and refuse removing one, and an index can use
//! it (D19).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Kind, ModelLock, Refusal, check};

/// Orders that reference customers, kept in another space, with an index
/// on the reference.
const LOCK: &str = r#"{
  "collections": {
    "customers": {
      "fields": {
        "name": { "rule": 1, "since": 1, "type": "ncdb/string" }
      },
      "since": 1
    },
    "orders": {
      "fields": {
        "customer": { "rule": 1, "since": 1, "targets": ["customers"], "type": "ncdb/ref" }
      },
      "indexes": {
        "byCustomer": {
          "equal": 1,
          "fields": [{ "direction": "asc", "field": "customer" }],
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

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).expect("the lock reads")
}

#[test]
fn reads_a_reference_with_its_targets() {
    let lock = lock();
    let customer = lock.field("orders", "customer").unwrap();
    assert_eq!(customer.kind, Kind::Ref(vec!["customers".into()]));
    assert_eq!(customer.kind.type_id(), "ncdb/ref");
    assert_eq!(customer.kind.targets(), Some(&["customers".to_owned()][..]));
    assert_eq!(customer.rule, 1);
}

#[test]
fn refuses_a_reference_without_targets_and_names_where() {
    let text = LOCK.replace(r#""targets": ["customers"], "#, "");
    let error = ModelLock::from_json(&text).unwrap_err();
    assert_eq!(error.path, "collections.orders.fields.customer.targets");
}

#[test]
fn writes_a_reference_back_as_it_was_read() {
    let lock = lock();
    assert_eq!(
        lock.to_json(),
        serde_json::from_str::<serde_json::Value>(LOCK).unwrap()
    );
}

#[test]
fn an_index_can_use_a_reference() {
    assert!(Kind::Ref(vec!["customers".into()]).is_indexable());
    assert_eq!(check::validate(&lock()), vec![]);
}

#[test]
fn a_reference_is_not_a_basic_value_and_not_a_link() {
    let kind = Kind::Ref(vec!["customers".into()]);
    // It cannot be personal (D22), nor name the person of a personal field.
    assert!(!kind.is_basic());
    let text = LOCK.replace(
        r#""customer": {"#,
        r#""note": { "personal": { "person": "customer" }, "rule": 1, "since": 1, "type": "ncdb/string" },
        "customer": {"#,
    );
    let refusals = check::validate(&ModelLock::from_json(&text).unwrap());
    assert_eq!(
        refusals,
        vec![Refusal::PersonNotLink {
            collection: "orders".into(),
            field: "note".into(),
            person: "customer".into(),
        }]
    );
}

#[test]
fn refuses_a_target_that_is_no_collection_of_the_model() {
    let text = LOCK.replace(r#"["customers"]"#, r#"["custmers"]"#);
    let refusals = check::validate(&ModelLock::from_json(&text).unwrap());
    assert_eq!(
        refusals,
        vec![Refusal::UnknownTarget {
            collection: "orders".into(),
            field: "customer".into(),
            target: "custmers".into(),
        }]
    );
}

/// The lock at `version`, with the reference's `targets`; every lock
/// has the collection `suppliers`.
fn with_targets(version: u64, targets: &str) -> ModelLock {
    let text = LOCK.replace(r#"["customers"]"#, targets).replace(
        r#""orders": {"#,
        r#""suppliers": { "fields": {}, "since": 1 },
    "orders": {"#,
    );
    let mut lock = ModelLock::from_json(&text).unwrap();
    lock.model.version = version;
    lock.model.hash = lock.compute_hash();
    lock
}

#[test]
fn adding_a_target_is_accepted_and_removing_one_is_refused() {
    let v1 = with_targets(1, r#"["customers"]"#);
    let v2 = with_targets(2, r#"["customers", "suppliers"]"#);
    assert_eq!(check::successor(&v1, &v2), vec![]);

    let v3 = with_targets(3, r#"["customers"]"#);
    let removed = Refusal::TargetRemoved {
        collection: "orders".into(),
        field: "customer".into(),
        target: "suppliers".into(),
    };
    assert_eq!(check::successor(&v2, &v3), vec![removed.clone()]);
    // The reason names what upgraded clients would read: a reference into
    // a collection the field no longer lists reads missing.
    let lines = removed.lines().join("\n");
    assert!(lines.contains("references"), "{lines}");
    assert!(lines.contains("missing"), "{lines}");
}
