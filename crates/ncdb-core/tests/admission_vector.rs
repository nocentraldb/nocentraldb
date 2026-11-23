//! The admission of RFC 0015 ("The admission"), against the vectors of
//! docs/rfc/vectors/0015/, which generate.py writes with cbor2, hashlib and
//! cryptography's Ed25519, independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::admission::{Admission, PREFIX, ReadError};
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::keys::RecoveryKey;

const ADMISSION_HEX: &str = include_str!("../../../docs/rfc/vectors/0015/admission.hex");
const ADMISSION_TEXT: &str = include_str!("../../../docs/rfc/vectors/0015/admission.txt");
const ACCOUNT_KEY_HEX: &str =
    include_str!("../../../docs/rfc/vectors/0015/admission-account-key.hex");
const ACCOUNT_KEY_TEXT: &str =
    include_str!("../../../docs/rfc/vectors/0015/admission-account-key.txt");
const FIELDS: &str = include_str!("../../../docs/rfc/vectors/0015/fields.txt");

const UNTIL: u64 = 1_798_761_600;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// A field of fields.txt, by name.
fn field(name: &str) -> Vec<u8> {
    let line = FIELDS
        .lines()
        .find(|line| line.split(": ").next() == Some(name))
        .unwrap_or_else(|| panic!("fields.txt has no {name}"));
    hex(line.split(": ").nth(1).unwrap())
}

fn admission_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

fn relay_id() -> Vec<u8> {
    (0..16).collect()
}

fn recovery_key() -> [u8; 32] {
    RecoveryKey::from_code(&std::array::from_fn(|i| 0xa0 + i as u8)).public()
}

fn account_key() -> [u8; 32] {
    SigningKey::from_bytes(&[0x11; 32])
        .verifying_key()
        .to_bytes()
}

fn public() -> [u8; 32] {
    admission_key().verifying_key().to_bytes()
}

/// The vector's map with one entry changed, or removed with `None`.
fn changed(key: u64, value: Option<Value>) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&hex(ADMISSION_HEX)).unwrap() else {
        panic!("an admission is a map");
    };
    match value {
        Some(value) => map.insert(Key::Unsigned(key), value),
        None => map.remove(&Key::Unsigned(key)),
    };
    cbor::encode(&Value::Map(map))
}

#[test]
fn the_inputs_are_the_vectors() {
    assert_eq!(Admission::CONTEXT, b"nocentraldb/relay-admission");
    assert_eq!(PREFIX, "ncdb-admit-1:");
    assert_eq!(public().to_vec(), field("admission key"));
    assert_eq!(relay_id(), field("relay id"));
    assert_eq!(recovery_key().to_vec(), field("recovery key"));
    assert_eq!(account_key().to_vec(), field("account key"));
}

#[test]
fn an_admission_signed_here_equals_the_vector() {
    let admission = Admission::sign(&admission_key(), relay_id(), recovery_key(), UNTIL, 20);
    assert_eq!(admission.digest().to_vec(), field("digest"));
    assert_eq!(admission.encode(), hex(ADMISSION_HEX));
    assert_eq!(admission.to_string(), ADMISSION_TEXT.trim());
}

#[test]
fn an_admission_of_an_account_key_equals_the_vector() {
    let admission = Admission::sign(&admission_key(), relay_id(), account_key(), UNTIL, 1);
    assert_eq!(admission.digest().to_vec(), field("account key digest"));
    assert_eq!(admission.encode(), hex(ACCOUNT_KEY_HEX));
    assert_eq!(admission.to_string(), ACCOUNT_KEY_TEXT.trim());
}

#[test]
fn the_vector_decodes_with_its_fields_and_verifies() {
    let admission = Admission::decode(&hex(ADMISSION_HEX)).unwrap();
    assert_eq!(admission.relay_id, relay_id());
    assert_eq!(admission.identity, recovery_key());
    assert_eq!(admission.until, UNTIL);
    assert_eq!(admission.spaces, 20);
    assert!(admission.verify(&public()));
    assert_eq!(admission.encode(), hex(ADMISSION_HEX));
}

#[test]
fn the_text_reads_as_the_bytes() {
    let admission: Admission = ADMISSION_TEXT.trim().parse().unwrap();
    assert_eq!(admission.encode(), hex(ADMISSION_HEX));
    let other: Admission = ACCOUNT_KEY_TEXT.trim().parse().unwrap();
    assert_eq!(other.encode(), hex(ACCOUNT_KEY_HEX));
}

#[test]
fn another_key_does_not_verify_it() {
    let admission = Admission::decode(&hex(ADMISSION_HEX)).unwrap();
    let other = SigningKey::from_bytes(&[0x5b; 32])
        .verifying_key()
        .to_bytes();
    assert!(!admission.verify(&other));
}

#[test]
fn a_changed_field_breaks_the_signature() {
    for (key, value) in [
        (1, Value::Bytes(vec![0xff; 16])),
        (2, Value::Bytes(account_key().to_vec())),
        (3, Value::Unsigned(UNTIL + 1)),
        (4, Value::Unsigned(21)),
    ] {
        let admission = Admission::decode(&changed(key, Some(value))).unwrap();
        assert!(!admission.verify(&public()), "key {key}");
    }
}

#[test]
fn a_map_without_a_key_or_with_a_wrong_one_is_refused() {
    for key in 1..=5 {
        assert!(Admission::decode(&changed(key, None)).is_err(), "without key {key}");
    }
    for (key, value) in [
        (1, Value::Unsigned(1)),
        (2, Value::Bytes(vec![0; 16])),
        (3, Value::Bytes(vec![0; 8])),
        (4, Value::Bytes(vec![0; 8])),
        (5, Value::Bytes(vec![0; 63])),
        (6, Value::Unsigned(1)),
    ] {
        assert!(
            Admission::decode(&changed(key, Some(value))).is_err(),
            "key {key}"
        );
    }
}

#[test]
fn bytes_that_are_not_canonical_cbor_are_refused() {
    let mut bytes = hex(ADMISSION_HEX);
    bytes.push(0);
    assert!(Admission::decode(&bytes).is_err());
    assert!(Admission::decode(&[]).is_err());
}

#[test]
fn a_text_without_the_prefix_or_in_another_encoding_is_refused() {
    let text = ADMISSION_TEXT.trim();
    let body = text.strip_prefix(PREFIX).unwrap();
    assert_eq!(
        format!("ncdb-admit-2:{body}").parse::<Admission>(),
        Err(ReadError::Prefix)
    );
    assert_eq!(
        format!("{PREFIX}{body}=").parse::<Admission>(),
        Err(ReadError::Encoding)
    );
    assert_eq!(
        format!("{PREFIX}{}", body.replace('_', "/")).parse::<Admission>(),
        Err(ReadError::Encoding)
    );
    assert_eq!(
        format!("{PREFIX}AAAA").parse::<Admission>(),
        Err(ReadError::Map)
    );
}

#[test]
fn its_debug_output_shows_no_signature() {
    let admission = Admission::decode(&hex(ADMISSION_HEX)).unwrap();
    let shown = format!("{admission:?}");
    assert!(shown.contains("spaces: 20"), "{shown}");
    assert!(!shown.contains("signature"), "{shown}");
}
