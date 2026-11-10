//! The recovery key and the account key statement of RFC 0003 ("Recovery
//! key", "Account key statement"), against the vectors of
//! tests/vectors/membership/, which generate.py writes with cbor2 and
//! cryptography's HKDF and Ed25519, independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::AccountKeyStatement;

const RECOVERY_PUBLIC: &str = include_str!("vectors/membership/recovery-public.hex");
const STATEMENT: &str = include_str!("vectors/membership/statement.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The recovery code's 128 bits of the vector.
fn code() -> [u8; 16] {
    std::array::from_fn(|i| 0xa0 + i as u8)
}

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&code())
}

fn account_key() -> [u8; 32] {
    SigningKey::from_bytes(&[0x11; 32])
        .verifying_key()
        .to_bytes()
}

/// The statement's map with one entry changed.
fn changed(key: u64, value: Value) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&hex(STATEMENT)).unwrap() else {
        panic!("a statement is a map");
    };
    map.insert(Key::Unsigned(key), value);
    cbor::encode(&Value::Map(map))
}

#[test]
fn the_recovery_key_derives_from_the_code_as_the_vector() {
    assert_eq!(RecoveryKey::CONTEXT, b"nocentraldb/recovery-key");
    assert_eq!(recovery().public().to_vec(), hex(RECOVERY_PUBLIC));
}

#[test]
fn another_code_derives_another_recovery_key() {
    let mut other = code();
    other[15] ^= 1;
    assert_ne!(RecoveryKey::from_code(&other).public(), recovery().public());
}

#[test]
fn a_recovery_key_does_not_show_its_secret() {
    let shown = format!("{:?}", recovery());
    assert!(shown.contains("redacted"), "{shown}");
}

#[test]
fn a_statement_signed_here_equals_the_vector() {
    let statement = AccountKeyStatement::sign(&recovery(), account_key(), 1);
    assert_eq!(statement.encode(), hex(STATEMENT));
}

#[test]
fn the_statement_decodes_encodes_and_verifies_as_the_vector() {
    let bytes = hex(STATEMENT);
    let statement = AccountKeyStatement::decode(&bytes).unwrap();
    assert_eq!(statement.encode(), bytes);
    assert_eq!(statement.recovery_key.to_vec(), hex(RECOVERY_PUBLIC));
    assert_eq!(statement.account_key, account_key());
    assert_eq!(statement.number, 1);
    assert!(statement.verify());
    assert_eq!(
        AccountKeyStatement::CONTEXT,
        b"nocentraldb/account-key-statement"
    );
}

#[test]
fn a_statement_with_a_changed_field_does_not_verify() {
    let statement = AccountKeyStatement::decode(&hex(STATEMENT)).unwrap();
    let mut other_number = statement.clone();
    other_number.number = 2;
    assert!(!other_number.verify());
    let mut other_account = statement.clone();
    other_account.account_key[0] ^= 1;
    assert!(!other_account.verify());
    let mut other_signature = statement.clone();
    other_signature.signature[0] ^= 1;
    assert!(!other_signature.verify());
}

#[test]
fn a_statement_signed_by_another_key_does_not_verify() {
    // Signed by a key that is not the one key 1 names.
    let mut other = code();
    other[0] ^= 1;
    let mut statement =
        AccountKeyStatement::sign(&RecoveryKey::from_code(&other), account_key(), 1);
    statement.recovery_key = recovery().public();
    assert!(!statement.verify());
}

#[test]
fn a_statement_with_number_zero_does_not_verify() {
    // Its fields are public, so a statement with number 0 can be built in
    // memory with a valid signature by the recovery key; it still does not
    // hold. The seed is derived here as RFC 0003 gives it.
    let mut seed = [0; 32];
    hkdf::Hkdf::<sha2::Sha256>::new(None, &code())
        .expand(b"nocentraldb/recovery-key", &mut seed)
        .unwrap();
    let signer = SigningKey::from_bytes(&seed);
    assert_eq!(signer.verifying_key().to_bytes(), recovery().public());
    let mut statement = AccountKeyStatement::sign(&recovery(), account_key(), 1);
    statement.number = 0;
    statement.signature = ed25519_dalek::Signer::sign(&signer, &statement.digest()).to_bytes();
    assert!(
        signer
            .verifying_key()
            .verify_strict(
                &statement.digest(),
                &ed25519_dalek::Signature::from_bytes(&statement.signature)
            )
            .is_ok(),
        "the signature itself is valid"
    );
    assert!(!statement.verify());
}

#[test]
fn a_statement_with_number_zero_does_not_decode() {
    // RFC 0003: 1 for the first account key, one more with each new one.
    assert!(AccountKeyStatement::decode(&changed(3, Value::Unsigned(0))).is_err());
}

#[test]
fn a_statement_with_another_key_does_not_decode() {
    // RFC 0003 defines no extensions for it, and keys 4 to 63 are
    // must-understand.
    assert!(AccountKeyStatement::decode(&changed(5, Value::Unsigned(1))).is_err());
}

#[test]
fn a_statement_with_a_short_field_does_not_decode() {
    assert!(AccountKeyStatement::decode(&changed(1, Value::Bytes(vec![0; 31]))).is_err());
    assert!(AccountKeyStatement::decode(&changed(4, Value::Bytes(vec![0; 63]))).is_err());
}
