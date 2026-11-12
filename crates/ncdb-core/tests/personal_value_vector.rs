//! A personal value sealed under the person key, as the owner decided on
//! NOC-244 (Carsten, 2026-11-12) and RFC 0003 gives it ("Personal value"):
//! nonce (24) || ciphertext || tag (16), XChaCha20-Poly1305 under the
//! HKDF-SHA256 subkey of the person key, with the space, object and
//! person-key ids and the collection and field names as associated data,
//! against the vectors docs/rfc/vectors/0003/personal_value.py writes with
//! cbor2, cryptography and PyNaCl, independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::keys::PersonKey;
use ncdb_core::personal::{self, OpenError, Person, Place, Sealed};

const AD: &str = include_str!("../../../docs/rfc/vectors/0003/personal-value.ad.hex");
const PLAINTEXT: &str = include_str!("../../../docs/rfc/vectors/0003/personal-value.plaintext.hex");
const SEALED: &str = include_str!("../../../docs/rfc/vectors/0003/personal-value.sealed.hex");
const OPERATION: &str = include_str!("../../../docs/rfc/vectors/0003/personal-value.operation.hex");
const PERSON: &str = include_str!("../../../docs/rfc/vectors/0003/person.operation.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn range<const N: usize>(start: u8) -> [u8; N] {
    std::array::from_fn(|i| start + i as u8)
}

const PERSON_KEY_ID: u8 = 0x61;

fn place(field: &str) -> Place<'_> {
    Place {
        space: SpaceId(range(0x10)),
        object: ObjectId(range(0x30)),
        person_key: range(PERSON_KEY_ID),
        collection: "people",
        field,
    }
}

fn key() -> PersonKey {
    PersonKey::from_bytes([0x55; 32])
}

fn value() -> Value {
    Value::Text("Anna Schmidt".into())
}

/// Argument 1 of the operation in `hex`: key 5 of the operation, then key 1.
fn argument_1(hex_text: &str) -> Value {
    let Value::Map(operation) = cbor::decode(&hex(hex_text)).unwrap() else {
        panic!("an operation is a map");
    };
    let Some(Value::Map(arguments)) = operation.get(&Key::Unsigned(5)) else {
        panic!("key 5 holds the arguments");
    };
    arguments[&Key::Unsigned(1)].clone()
}

#[test]
fn the_associated_data_is_the_vector() {
    assert_eq!(personal::associated_data(&place("name")), hex(AD));
}

#[test]
fn sealing_with_the_vectors_nonce_gives_the_vector() {
    let sealed = personal::seal_with_nonce(&key(), &place("name"), &value(), range(0xc1));
    assert_eq!(sealed, hex(SEALED));
    assert_eq!(&sealed[24..sealed.len() - 16].len(), &hex(PLAINTEXT).len());
}

#[test]
fn the_vector_opens_to_the_value() {
    let opened = personal::open(&key(), &place("name"), &hex(SEALED)).unwrap();
    assert_eq!(opened, value());
    assert_eq!(cbor::encode(&opened), hex(PLAINTEXT));
}

#[test]
fn a_random_nonce_seals_differently_and_opens() {
    let one = personal::seal(&key(), &place("name"), &value());
    let two = personal::seal(&key(), &place("name"), &value());
    assert_ne!(one, two);
    assert_eq!(personal::open(&key(), &place("name"), &one).unwrap(), value());
}

#[test]
fn moved_to_another_field_or_under_another_key_it_does_not_open() {
    let sealed = hex(SEALED);
    assert_eq!(
        personal::open(&key(), &place("email"), &sealed),
        Err(OpenError::DoesNotOpen)
    );
    let other = PersonKey::from_bytes([0x56; 32]);
    assert_eq!(
        personal::open(&other, &place("name"), &sealed),
        Err(OpenError::DoesNotOpen)
    );
    let mut moved = place("name");
    moved.object = ObjectId(range(0x31));
    assert_eq!(
        personal::open(&key(), &moved, &sealed),
        Err(OpenError::DoesNotOpen)
    );
}

#[test]
fn too_short_or_not_canonical_does_not_open() {
    assert_eq!(
        personal::open(&key(), &place("name"), &[0; 39]),
        Err(OpenError::TooShort)
    );
    // A plaintext that is not canonical CBOR: the integer 1 in two bytes.
    let sealed = personal::seal_bytes_with_nonce(&key(), &place("name"), &[0x18, 0x01], [7; 24]);
    assert_eq!(
        personal::open(&key(), &place("name"), &sealed),
        Err(OpenError::NotCanonical)
    );
}

#[test]
fn the_sealed_value_is_argument_1_of_the_vectors_set() {
    let argument = argument_1(OPERATION);
    let sealed = Sealed {
        person_key: range(PERSON_KEY_ID),
        bytes: hex(SEALED),
    };
    assert_eq!(sealed.to_value(), argument);
    assert_eq!(Sealed::from_value(&argument), Some(sealed));
    assert_eq!(Sealed::from_value(&Value::Null), None);
    assert_eq!(Sealed::from_value(&value()), None);
}

#[test]
fn the_person_is_argument_1_of_the_vectors_person_set() {
    let argument = argument_1(PERSON);
    let person = Person {
        person: range(0x70),
        person_key: range(PERSON_KEY_ID),
    };
    assert_eq!(person.to_value(), argument);
    assert_eq!(Person::from_value(&argument), Some(person));
    assert_eq!(Person::from_value(&Value::Null), None);
    assert_eq!(personal::PERSON_FIELD, "$person");
}
