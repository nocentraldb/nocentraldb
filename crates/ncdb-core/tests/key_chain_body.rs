//! A key chain entry in its control body (NOC-71): RFC 0008's entry type
//! 3 carries the new generation id (key 8), its previous generations, each
//! with that generation's key encrypted under the new one (key 9, sorted by
//! generation id), and the revocations the rotation answers (key 10).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::ChangeId;
use ncdb_core::membership::{self, BodyError, ControlBody, Entry, KeyChainEntry, PreviousKey};

const FIRST: u64 = (1 << 48) | 0x0102_0304_0506;
const OTHER: u64 = (1 << 48) | 0x0a0b_0c0d_0e0f;
const NEW: u64 = (2 << 48) | 0x1112_1314_1516;

fn entry() -> KeyChainEntry {
    KeyChainEntry {
        generation: NEW,
        previous: vec![
            PreviousKey {
                generation: FIRST,
                key: vec![0xa1; 72],
            },
            PreviousKey {
                generation: OTHER,
                key: vec![0xa2; 72],
            },
        ],
        answers: BTreeSet::from([ChangeId([5; 32])]),
    }
}

fn body() -> ControlBody {
    let deps = BTreeSet::from([ChangeId([4; 32])]);
    ControlBody::new(deps, Entry::KeyChain(entry()))
}

/// The body's map, changed by `edit`, encoded again.
fn changed(edit: impl FnOnce(&mut BTreeMap<Key, Value>)) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&body().encode()).unwrap() else {
        panic!("a control body is a map");
    };
    edit(&mut map);
    cbor::encode(&Value::Map(map))
}

fn malformed(bytes: &[u8]) -> bool {
    matches!(ControlBody::decode(bytes), Err(BodyError::Malformed(_)))
}

fn previous(items: &[(u64, Value)]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|(generation, key)| {
                Value::Map(BTreeMap::from([
                    (Key::Unsigned(1), Value::Unsigned(*generation)),
                    (Key::Unsigned(2), key.clone()),
                ]))
            })
            .collect(),
    )
}

#[test]
fn a_key_chain_entry_is_entry_type_3_with_its_fields_under_keys_8_to_10() {
    let Value::Map(map) = cbor::decode(&body().encode()).unwrap() else {
        panic!("a control body is a map");
    };
    let keys: Vec<&Key> = map.keys().collect();
    let expected = [1, 3, 8, 9, 10].map(Key::Unsigned);
    assert_eq!(keys, expected.iter().collect::<Vec<_>>());
    assert_eq!(
        map[&Key::Unsigned(1)],
        Value::Unsigned(membership::KEY_CHAIN)
    );
    assert_eq!(map[&Key::Unsigned(8)], Value::Unsigned(NEW));
    assert_eq!(
        map[&Key::Unsigned(9)],
        previous(&[
            (FIRST, Value::Bytes(vec![0xa1; 72])),
            (OTHER, Value::Bytes(vec![0xa2; 72])),
        ])
    );
    assert_eq!(
        map[&Key::Unsigned(10)],
        Value::Array(vec![Value::Bytes(vec![5; 32])])
    );
}

#[test]
fn a_key_chain_entry_decodes_to_what_was_encoded() {
    let decoded = ControlBody::decode(&body().encode()).unwrap();
    assert_eq!(decoded, body());
    assert_eq!(decoded.entry_type(), membership::KEY_CHAIN);
    let Entry::KeyChain(chain) = decoded.entry else {
        panic!("entry type 3 reads as a key chain entry");
    };
    assert_eq!(chain.previous_generations().collect::<Vec<_>>(), [FIRST, OTHER]);
}

#[test]
fn a_rotation_that_answers_no_revocation_has_an_empty_key_10() {
    let mut chain = entry();
    chain.answers.clear();
    let body = ControlBody::new(BTreeSet::from([ChangeId([4; 32])]), Entry::KeyChain(chain));
    assert_eq!(ControlBody::decode(&body.encode()), Ok(body));
}

#[test]
fn a_missing_or_mistyped_key_chain_field_is_malformed() {
    for key in [8, 9, 10] {
        let missing = changed(|m| {
            m.remove(&Key::Unsigned(key));
        });
        assert!(malformed(&missing), "key {key} missing");
        let text = changed(|m| {
            m.insert(Key::Unsigned(key), Value::Text("1".into()));
        });
        assert!(malformed(&text), "key {key} as text");
    }
}

#[test]
fn previous_generations_are_sorted_unique_and_not_empty() {
    let key = || Value::Bytes(vec![0xa1; 72]);
    for (name, items) in [
        ("empty", vec![]),
        ("unsorted", vec![(OTHER, key()), (FIRST, key())]),
        ("twice", vec![(FIRST, key()), (FIRST, key())]),
        ("key not bytes", vec![(FIRST, Value::Unsigned(1))]),
    ] {
        let bytes = changed(|m| {
            m.insert(Key::Unsigned(9), previous(&items));
        });
        assert!(malformed(&bytes), "{name}");
    }
}

#[test]
fn a_previous_generation_is_a_map_of_exactly_keys_1_and_2() {
    let extra = Value::Array(vec![Value::Map(BTreeMap::from([
        (Key::Unsigned(1), Value::Unsigned(FIRST)),
        (Key::Unsigned(2), Value::Bytes(vec![1; 72])),
        (Key::Unsigned(3), Value::Null),
    ]))]);
    let missing = Value::Array(vec![Value::Map(BTreeMap::from([(
        Key::Unsigned(1),
        Value::Unsigned(FIRST),
    )]))]);
    for (name, value) in [("extra key", extra), ("missing key", missing)] {
        let bytes = changed(|m| {
            m.insert(Key::Unsigned(9), value);
        });
        assert!(malformed(&bytes), "{name}");
    }
}

#[test]
fn unsorted_answers_are_malformed() {
    let bytes = changed(|m| {
        m.insert(
            Key::Unsigned(10),
            Value::Array(vec![Value::Bytes(vec![6; 32]), Value::Bytes(vec![5; 32])]),
        );
    });
    assert!(malformed(&bytes));
}

#[test]
fn a_key_from_11_to_63_in_a_key_chain_entry_is_not_understood() {
    for key in [11, 63] {
        let newer = changed(|m| {
            m.insert(Key::Unsigned(key), Value::Null);
        });
        assert_eq!(
            ControlBody::decode(&newer),
            Err(BodyError::NotUnderstood),
            "key {key}"
        );
    }
}

#[test]
fn the_debug_output_shows_generations_but_no_encrypted_key() {
    let shown = format!("{:?}", entry());
    assert!(shown.contains(&NEW.to_string()), "{shown}");
    assert!(!shown.contains("161, 161"), "{shown}");
}
