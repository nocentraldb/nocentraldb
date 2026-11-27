//! `nc.ref` over a link (RFC 0018, "One function, two cases" and "Existing
//! link fields read as references"): `ncdb model build` locks a reference
//! that is the `person` of a personal field as `ncdb/link`, and keeps a
//! field already locked as `ncdb/link` when the schema now writes `nc.ref`
//! for it, so neither the lock nor the model hash changes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{Draft, Kind, ModelId, ModelLock, Refusal, build};

/// Tasks with an assignee and a personal note about the assignee; `{type}`
/// is the assignee's type in the schema.
fn schema(assignee: &str, personal: bool) -> Draft {
    let note = match personal {
        true => r#", "note": { "personal": { "person": "assignee" }, "rule": 1, "type": "ncdb/string" }"#,
        false => "",
    };
    let text = format!(
        r#"{{
  "collections": {{
    "people": {{ "fields": {{ "name": {{ "rule": 1, "type": "ncdb/string" }} }} }},
    "tasks": {{
      "fields": {{
        "assignee": {{ "rule": 1, "targets": ["people"], "type": "{assignee}" }}{note}
      }},
      "indexes": {{
        "byAssignee": {{ "equal": 1, "fields": [{{ "direction": "asc", "field": "assignee" }}] }}
      }}
    }}
  }},
  "plugins": {{}}
}}"#
    );
    Draft::from_json(&text).expect("the schema reads")
}

fn id() -> ModelId {
    ModelId([7; 16])
}

fn first(draft: &Draft) -> ModelLock {
    build(draft, None, id).expect("the first build").lock
}

fn assignee(lock: &ModelLock) -> &Kind {
    &lock.field("tasks", "assignee").unwrap().kind
}

#[test]
fn a_reference_that_names_the_person_of_a_personal_field_is_locked_as_a_link() {
    let lock = first(&schema("ncdb/ref", true));
    assert_eq!(assignee(&lock), &Kind::Link(vec!["people".into()]));
    assert_eq!(lock.field("tasks", "assignee").unwrap().rule, 1);
}

#[test]
fn the_lock_is_the_one_nc_link_would_build_byte_for_byte() {
    let from_ref = first(&schema("ncdb/ref", true));
    let from_link = first(&schema("ncdb/link", true));
    assert_eq!(from_ref.to_json(), from_link.to_json());
}

#[test]
fn a_reference_that_names_no_person_stays_a_reference() {
    let lock = first(&schema("ncdb/ref", false));
    assert_eq!(assignee(&lock), &Kind::Ref(vec!["people".into()]));
}

#[test]
fn nc_ref_over_a_field_locked_as_a_link_keeps_the_lock_and_the_model_hash() {
    let previous = first(&schema("ncdb/link", false));
    let next = build(&schema("ncdb/ref", false), Some(&previous), id)
        .expect("the build accepts nc.ref over ncdb/link")
        .lock;
    assert_eq!(next.to_json(), previous.to_json());
    assert_eq!(next.model.hash, previous.model.hash);
    assert_eq!(next.model.version, previous.model.version);
}

#[test]
fn nc_ref_over_a_person_locked_as_a_link_keeps_the_lock_too() {
    let previous = first(&schema("ncdb/link", true));
    let next = build(&schema("ncdb/ref", true), Some(&previous), id)
        .expect("the build accepts it")
        .lock;
    assert_eq!(next.to_json(), previous.to_json());
}

#[test]
fn a_field_locked_as_a_reference_that_becomes_a_person_is_refused() {
    // The field already holds references, possibly into other spaces; its
    // type cannot change (RFC 0005, "Refused"), so it cannot be a person.
    let previous = first(&schema("ncdb/ref", false));
    let refusals = build(&schema("ncdb/ref", true), Some(&previous), id).unwrap_err();
    assert_eq!(
        refusals,
        vec![Refusal::PersonNotLink {
            collection: "tasks".into(),
            field: "note".into(),
            person: "assignee".into(),
        }]
    );
}
