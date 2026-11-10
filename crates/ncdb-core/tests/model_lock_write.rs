//! Writing `model.lock.json` in its one canonical form (RFC 0005,
//! "Canonical form") and its model hash ("The keys", `model.hash`).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{ModelLock, write::canonical};
use serde_json::json;

const EXAMPLE: &str = include_str!("model_lock/rfc-0005-v2.lock.json");

fn example() -> ModelLock {
    ModelLock::from_json(EXAMPLE).unwrap()
}

#[test]
fn writes_the_rfc_example_byte_for_byte() {
    assert_eq!(example().to_canonical_json(), EXAMPLE);
}

#[test]
fn the_hash_of_the_rfc_example_is_the_one_the_rfc_gives() {
    assert_eq!(
        example().compute_hash(),
        "sha256:a59fed6588bc432d392c671b9ce7816babffc1fd7208187e4f2dd2c952e0531d"
    );
}

#[test]
fn the_hash_leaves_out_descriptions_and_the_model_entry() {
    let mut lock = example();
    let hash = lock.compute_hash();
    lock.collections
        .get_mut("tasks")
        .unwrap()
        .fields
        .get_mut("title")
        .unwrap()
        .description = Some("Something else.".into());
    lock.model.version = 9;
    assert_eq!(lock.compute_hash(), hash);
    lock.collections
        .get_mut("tasks")
        .unwrap()
        .fields
        .remove("archived");
    assert_ne!(lock.compute_hash(), hash);
}

#[test]
fn writes_what_it_reads() {
    let text = r#"{
      "collections": {
        "tasks": {
          "deprecated": 4,
          "deprecatedReason": "Use issues.",
          "fields": {
            "priority": { "deprecated": 3, "deprecatedReason": "Use rank.", "rule": 1, "since": 1, "type": "ncdb/int" },
            "kinds": { "of": { "options": ["bug", "feature"], "type": "ncdb/choice" }, "rule": 1, "since": 1, "type": "ncdb/set" },
            "votes": { "config": { "max": 1.5e300, "step": 1 }, "rule": 2, "since": 3, "type": "acme/counter" }
          },
          "since": 1
        }
      },
      "format": 1,
      "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 4 },
      "plugins": { "acme": { "crate": "acme-ncdb-counter", "npm": "@acme/ncdb-counter" } }
    }"#;
    let lock = ModelLock::from_json(text).unwrap();
    let written = lock.to_canonical_json();
    assert_eq!(ModelLock::from_json(&written).unwrap(), lock);
    assert_eq!(
        lock.to_json(),
        serde_json::from_str::<serde_json::Value>(text).unwrap()
    );
    assert!(
        written.contains("\"deprecatedReason\": \"Use rank.\""),
        "{written}"
    );
    assert!(
        written.contains("\"deprecatedReason\": \"Use issues.\""),
        "{written}"
    );
    assert!(
        written.contains("\"crate\": \"acme-ncdb-counter\""),
        "{written}"
    );
    assert!(written.ends_with("}\n") && !written.contains('\r'));
}

#[test]
fn short_objects_and_arrays_go_on_one_line_up_to_80_columns() {
    let written = example().to_canonical_json();
    // An object or array put on one line never makes the line longer than
    // 80 columns, trailing comma included.
    for line in written
        .lines()
        .filter(|l| l.ends_with("},") || l.ends_with('}') || l.ends_with("],") || l.ends_with(']'))
    {
        if line.contains('{') || line.contains('[') {
            assert!(line.chars().count() <= 80, "{line}");
        }
    }
    // An object that holds an object is never put on one line, even when it
    // would fit: "computed": { "blocked": { "since": 1 } } spans three lines.
    assert!(written.contains("\"computed\": {\n        \"blocked\": { \"since\": 1 }\n      },"));
}

#[test]
fn canonical_json_follows_rfc_8785() {
    // Keys sort by UTF-16 code units: U+E000 (one unit) sorts after
    // U+1F600 (a surrogate pair starting at 0xD83D), unlike in UTF-8.
    let value = json!({ "\u{e000}": 1, "\u{1f600}": 2, "b": [true, null], "a": "x" });
    assert_eq!(
        canonical(&value),
        "{\"a\":\"x\",\"b\":[true,null],\"\u{1f600}\":2,\"\u{e000}\":1}"
    );
    // Strings escape only quote, backslash and control characters.
    assert_eq!(canonical(&json!("a\"\\\n\u{1}é/")), r#""a\"\\\n\u0001é/""#);
    // Numbers as ECMAScript writes them.
    let numbers = json!([
        0,
        -0.0,
        1.0,
        1e21,
        1e20,
        1.5e-7,
        0.000001,
        123.456,
        -7,
        9007199254740993u64,
        1.7976931348623157e308
    ]);
    assert_eq!(
        canonical(&numbers),
        "[0,0,1,1e+21,100000000000000000000,1.5e-7,0.000001,123.456,-7,9007199254740992,1.7976931348623157e+308]"
    );
}
