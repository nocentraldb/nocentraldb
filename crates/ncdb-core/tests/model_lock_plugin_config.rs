//! A plugin field's `config` is part of its merge rule (RFC 0011, "A type
//! that is not registered"; RFC 0005, "Refused"; NOC-251): the lock check,
//! which `ncdb model build` and the device run, refuses a change of it, as
//! it refuses a change of type. A config that appears or disappears is a
//! change too; the same config in another key order is not.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{ModelLock, Refusal, check};

/// A lock at `version` whose `tasks` hold `votes` of `acme/counter`, with
/// this `config` or none, and `more` fields after it.
fn lock(version: u64, config: Option<&str>, more: &str) -> ModelLock {
    let config = config.map_or(String::new(), |c| format!(r#""config": {c}, "#));
    let text = format!(
        r#"{{ "collections": {{ "tasks": {{ "since": 1, "fields": {{
              "votes": {{ {config}"rule": 1, "since": 1, "type": "acme/counter" }}{more} }} }} }},
            "format": 1, "plugins": {{ "acme": {{ "npm": "@acme/x" }} }},
            "model": {{ "hash": "sha256:00", "id": "6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a", "version": {version} }} }}"#
    );
    ModelLock::from_json(&text).unwrap()
}

fn config_changed() -> Vec<Refusal> {
    vec![Refusal::ConfigChanged {
        collection: "tasks".into(),
        field: "votes".into(),
        type_id: "acme/counter".into(),
    }]
}

#[test]
fn refuses_changing_a_plugin_fields_config() {
    let before = lock(1, Some(r#"{ "step": 1 }"#), "");
    let after = lock(2, Some(r#"{ "step": 2 }"#), "");
    let found = check::successor(&before, &after);
    assert_eq!(found, config_changed());
    assert_eq!(
        found[0].lines(),
        vec![
            "tasks.votes: the config of acme/counter changes.".to_owned(),
            "The config is part of the type's merge rule: members would merge existing values differently.".to_owned(),
            "Add a new field with the new config instead, and deprecate tasks.votes.".to_owned(),
        ]
    );
}

#[test]
fn refuses_a_config_that_appears_or_disappears() {
    let none = lock(1, None, "");
    let some = lock(2, Some(r#"{ "step": 1 }"#), "");
    assert_eq!(check::successor(&none, &some), config_changed());

    let some = lock(1, Some(r#"{ "step": 1 }"#), "");
    let none = lock(2, None, "");
    assert_eq!(check::successor(&some, &none), config_changed());
}

#[test]
fn accepts_an_unchanged_config_in_any_key_order_and_a_new_field_with_one() {
    let before = lock(1, Some(r#"{ "step": 1, "max": 5 }"#), "");
    let after = lock(2, Some(r#"{ "max": 5, "step": 1 }"#), "");
    assert_eq!(check::successor(&before, &after), vec![]);

    let added = lock(
        2,
        Some(r#"{ "step": 1, "max": 5 }"#),
        r#", "stars": { "config": { "step": 2 }, "rule": 1, "since": 2, "type": "acme/counter" }"#,
    );
    assert_eq!(check::successor(&before, &added), vec![]);
}
