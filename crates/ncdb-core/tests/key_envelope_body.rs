//! A key envelope in its control body (NOC-78): RFC 0008's entry type 2
//! carries the recipient, the generation, HPKE's encapsulated key and the
//! ciphertext under keys 8 to 11, and the core reads them back as the
//! envelope `envelope::open` takes.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::envelope::{self, KeyEnvelope};
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{self, BodyError, ControlBody, Entry};

const SPACE: SpaceId = SpaceId([7; 16]);
const GENERATION: u64 = (1 << 48) | 0x0102_0304_0506;

fn recipient() -> MemberKey {
    MemberKey::from_secrets([1; 32], [2; 32])
}

fn sealed() -> KeyEnvelope {
    let key = SpaceKey::from_bytes([0x5a; 32]);
    envelope::seal_with_ephemeral(&key, SPACE, GENERATION, &recipient().public(), [9; 32]).unwrap()
}

fn body() -> ControlBody {
    let deps = BTreeSet::from([ChangeId([4; 32])]);
    ControlBody::new(deps, Entry::KeyEnvelope(sealed()))
}

/// The body's map, changed by `edit`, encoded again.
fn changed(edit: impl FnOnce(&mut std::collections::BTreeMap<Key, Value>)) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&body().encode()).unwrap() else {
        panic!("a control body is a map");
    };
    edit(&mut map);
    cbor::encode(&Value::Map(map))
}

#[test]
fn an_envelope_is_entry_type_2_with_its_fields_under_keys_8_to_11() {
    let bytes = body().encode();
    let Value::Map(map) = cbor::decode(&bytes).unwrap() else {
        panic!("a control body is a map");
    };
    let keys: Vec<&Key> = map.keys().collect();
    let expected = [1, 3, 8, 9, 10, 11].map(Key::Unsigned);
    assert_eq!(keys, expected.iter().collect::<Vec<_>>());
    assert_eq!(
        map[&Key::Unsigned(1)],
        Value::Unsigned(membership::KEY_ENVELOPE)
    );
    for (key, value) in sealed().fields() {
        assert_eq!(map[&key], value);
    }
}

#[test]
fn the_decoded_envelope_opens_for_its_recipient() {
    let decoded = ControlBody::decode(&body().encode()).unwrap();
    assert_eq!(decoded, body());
    assert_eq!(decoded.entry_type(), membership::KEY_ENVELOPE);
    let Entry::KeyEnvelope(sealed) = decoded.entry else {
        panic!("entry type 2 reads as a key envelope");
    };
    assert!(envelope::open(&sealed, SPACE, &recipient()).is_ok());
}

#[test]
fn a_missing_or_mistyped_envelope_field_is_malformed() {
    for key in [8, 9, 10, 11] {
        let missing = changed(|m| {
            m.remove(&Key::Unsigned(key));
        });
        assert!(
            matches!(ControlBody::decode(&missing), Err(BodyError::Malformed(_))),
            "key {key} missing"
        );
    }
    let short = changed(|m| {
        m.insert(Key::Unsigned(8), Value::Bytes(vec![1; 31]));
    });
    assert!(matches!(
        ControlBody::decode(&short),
        Err(BodyError::Malformed(_))
    ));
    let text = changed(|m| {
        m.insert(Key::Unsigned(9), Value::Text("1".into()));
    });
    assert!(matches!(
        ControlBody::decode(&text),
        Err(BodyError::Malformed(_))
    ));
}

#[test]
fn a_key_from_12_to_63_in_an_envelope_is_not_understood() {
    for key in [12, 63] {
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
