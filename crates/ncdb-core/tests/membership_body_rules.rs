//! What a control body may not be (RFC 0008, "The control bodies"): which
//! bodies are malformed, which are kept but not understood, and the keys a
//! newer client may add.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::membership::{BodyError, ControlBody};

const ADD_BODY: &str = include_str!("vectors/membership/add.body.hex");
const GENESIS_BODY: &str = include_str!("vectors/membership/genesis.body.hex");
const REMOVE_BODY: &str = include_str!("vectors/membership/remove.body.hex");
const SET_ROLE_ADMIN_BODY: &str = include_str!("vectors/membership/set-role-admin.body.hex");
const SET_ROLE_READER_BODY: &str = include_str!("vectors/membership/set-role-reader.body.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The map of a vector body.
fn map(vector: &str) -> BTreeMap<Key, Value> {
    match cbor::decode(&hex(vector)).unwrap() {
        Value::Map(map) => map,
        other => panic!("not a map: {other:?}"),
    }
}

/// The body of `vector` with `change` applied to its map.
fn changed(vector: &str, change: impl FnOnce(&mut BTreeMap<Key, Value>)) -> Vec<u8> {
    let mut map = map(vector);
    change(&mut map);
    cbor::encode(&Value::Map(map))
}

fn malformed(bytes: &[u8]) -> bool {
    matches!(ControlBody::decode(bytes), Err(BodyError::Malformed(_)))
}

fn not_understood(bytes: &[u8]) -> bool {
    ControlBody::decode(bytes) == Err(BodyError::NotUnderstood)
}

#[test]
fn a_body_without_its_required_keys_is_malformed() {
    for (vector, key) in [
        (ADD_BODY, 1),
        (ADD_BODY, 3),
        (ADD_BODY, 9),
        (ADD_BODY, 10),
        (ADD_BODY, 13),
        (ADD_BODY, 14),
        (REMOVE_BODY, 15),
        (SET_ROLE_ADMIN_BODY, 14),
        (GENESIS_BODY, 13),
    ] {
        let bytes = changed(vector, |m| {
            m.remove(&Key::Unsigned(key));
        });
        assert!(malformed(&bytes), "without key {key}");
    }
}

#[test]
fn a_key_the_action_does_not_use_is_malformed() {
    // A remove with a role, a reader's set role with a from generation, an
    // add with a cutoff.
    let with = |vector, key, value| {
        changed(vector, |m| {
            m.insert(Key::Unsigned(key), value);
        })
    };
    assert!(malformed(&with(REMOVE_BODY, 10, Value::Unsigned(1))));
    assert!(malformed(&with(
        SET_ROLE_READER_BODY,
        14,
        Value::Unsigned(1)
    )));
    assert!(malformed(&with(ADD_BODY, 15, Value::Unsigned(0))));
    assert!(
        malformed(&with(ADD_BODY, 8, Value::Unsigned(4))),
        "action 4"
    );
    assert!(malformed(&with(ADD_BODY, 10, Value::Unsigned(4))), "role 4");
}

#[test]
fn an_add_whose_keys_do_not_hash_to_its_subject_is_malformed() {
    let bytes = changed(ADD_BODY, |m| {
        m.insert(Key::Unsigned(9), Value::Bytes(vec![0; 32]));
    });
    assert!(malformed(&bytes));
}

#[test]
fn dependencies_are_sorted_and_only_a_genesis_has_none() {
    let unsorted = changed(ADD_BODY, |m| {
        let ids = vec![Value::Bytes(vec![2; 32]), Value::Bytes(vec![1; 32])];
        m.insert(Key::Unsigned(3), Value::Array(ids));
    });
    assert!(malformed(&unsorted));
    let none = changed(ADD_BODY, |m| {
        m.insert(Key::Unsigned(3), Value::Array(vec![]));
    });
    assert!(malformed(&none));
    let genesis_with_one = changed(GENESIS_BODY, |m| {
        m.insert(
            Key::Unsigned(3),
            Value::Array(vec![Value::Bytes(vec![1; 32])]),
        );
    });
    assert!(malformed(&genesis_with_one));
}

#[test]
fn a_reserved_key_an_unknown_key_or_entry_type_is_not_understood() {
    for key in [0, 2, 4, 7, 20, 63] {
        let bytes = changed(ADD_BODY, |m| {
            m.insert(Key::Unsigned(key), Value::Null);
        });
        assert!(not_understood(&bytes), "key {key}");
    }
    let newer = changed(ADD_BODY, |m| {
        m.insert(Key::Unsigned(1), Value::Unsigned(7));
    });
    assert!(not_understood(&newer));
}

#[test]
fn keys_from_64_up_are_kept_and_encoded_again() {
    let bytes = changed(ADD_BODY, |m| {
        m.insert(Key::Unsigned(64), Value::Text("newer".into()));
    });
    let body = ControlBody::decode(&bytes).unwrap();
    assert_eq!(
        body.extensions().map(|(key, _)| key).collect::<Vec<_>>(),
        [64]
    );
    assert_eq!(body.encode(), bytes);

    let mut plain = ControlBody::decode(&hex(ADD_BODY)).unwrap();
    assert!(!plain.insert_extension(63, Value::Null));
    assert!(plain.insert_extension(64, Value::Text("newer".into())));
    assert_eq!(plain.encode(), bytes);
}

#[test]
fn the_other_entry_types_keep_their_fields() {
    let bytes = changed(REMOVE_BODY, |m| {
        m.insert(Key::Unsigned(1), Value::Unsigned(6));
        m.insert(Key::Unsigned(10), Value::Bool(true));
    });
    let body = ControlBody::decode(&bytes).unwrap();
    assert_eq!(body.entry_type(), 6);
    assert_eq!(body.encode(), bytes);
}
