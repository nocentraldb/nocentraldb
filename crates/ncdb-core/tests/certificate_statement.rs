//! The member certificate's key 6, the account key statement (RFC 0003,
//! "Member certificate"), against the vectors of tests/vectors/membership/,
//! which generate.py writes with cbor2 and cryptography, independently of
//! this crate. Key 5's signature does not cover key 6; a sub-agent's
//! certificate carries its parent's account's statement.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::keys::{MemberKey, RecoveryKey};
use ncdb_core::membership::{AccountKeyStatement, Certificate, MemberKind};

const CERTIFICATE: &str = include_str!("vectors/membership/certificate.hex");
const SUB_AGENT_CERTIFICATE: &str = include_str!("vectors/membership/sub-agent-certificate.hex");
const STATEMENT: &str = include_str!("vectors/membership/statement.hex");
const WITH_STATEMENT: &str = include_str!("vectors/membership/certificate-with-statement.hex");
const SUB_AGENT_WITH_STATEMENT: &str =
    include_str!("vectors/membership/sub-agent-certificate-with-statement.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn creator() -> MemberKey {
    MemberKey::from_secrets([0x21; 32], [0x22; 32])
}

fn sub_agent() -> MemberKey {
    MemberKey::from_secrets([0x71; 32], [0x72; 32])
}

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&std::array::from_fn(|i| 0xa0 + i as u8))
}

fn statement() -> AccountKeyStatement {
    AccountKeyStatement::decode(&hex(STATEMENT)).unwrap()
}

/// The bytes of a certificate map with key 6 set to `value`.
fn with_key_6(certificate: &str, value: Value) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&hex(certificate)).unwrap() else {
        panic!("a certificate is a map");
    };
    map.insert(Key::Unsigned(6), value);
    cbor::encode(&Value::Map(map))
}

#[test]
fn a_certificate_with_a_statement_signed_here_equals_the_vector() {
    let certificate = Certificate::sign(&account(), creator().id(), MemberKind::Agent)
        .with_statement(statement());
    assert_eq!(certificate.encode(), hex(WITH_STATEMENT));
}

#[test]
fn the_vector_decodes_encodes_and_verifies_with_its_statement() {
    let bytes = hex(WITH_STATEMENT);
    let certificate = Certificate::decode(&bytes).unwrap();
    assert_eq!(certificate.encode(), bytes);
    assert_eq!(certificate.statement, Some(statement()));
    assert_eq!(certificate.statement.as_ref().unwrap().recovery_key, recovery().public());
    assert!(certificate.verify(&certificate.account_key));
}

#[test]
fn key_5_signs_the_certificate_without_key_6() {
    let with = Certificate::decode(&hex(WITH_STATEMENT)).unwrap();
    let without = Certificate::decode(&hex(CERTIFICATE)).unwrap();
    assert_eq!(with.signature, without.signature);
    assert_eq!(with.digest(), without.digest());
}

#[test]
fn a_certificate_without_key_6_still_decodes_and_verifies() {
    let certificate = Certificate::decode(&hex(CERTIFICATE)).unwrap();
    assert_eq!(certificate.statement, None);
    assert_eq!(certificate.encode(), hex(CERTIFICATE));
    assert!(certificate.verify(&certificate.account_key));
}

#[test]
fn a_sub_agents_certificate_carries_its_parents_account_statement() {
    let parent = Certificate::decode(&hex(WITH_STATEMENT)).unwrap();
    let signed = Certificate::sign_sub_agent(&creator(), parent.account_key, sub_agent().id())
        .with_statement(parent.statement.clone().unwrap());
    let bytes = hex(SUB_AGENT_WITH_STATEMENT);
    assert_eq!(signed.encode(), bytes);

    let decoded = Certificate::decode(&bytes).unwrap();
    assert_eq!(decoded, signed);
    assert_eq!(decoded.statement, parent.statement);
    // The parent's signing key signs it, as without key 6.
    assert!(decoded.verify(&creator().public().signing));
    assert_eq!(
        decoded.signature,
        Certificate::decode(&hex(SUB_AGENT_CERTIFICATE)).unwrap().signature
    );
}

#[test]
fn a_statement_that_names_another_account_key_does_not_verify() {
    let other = AccountKeyStatement::sign(&recovery(), [0x99; 32], 1);
    assert!(other.verify());
    let certificate = Certificate::sign(&account(), creator().id(), MemberKind::Agent)
        .with_statement(other);
    assert!(!certificate.verify(&certificate.account_key));
}

#[test]
fn a_statement_whose_signature_fails_does_not_verify() {
    let mut broken = statement();
    broken.signature[0] ^= 1;
    let certificate = Certificate::sign(&account(), creator().id(), MemberKind::Agent)
        .with_statement(broken);
    assert!(!certificate.verify(&certificate.account_key));

    let mut sub = Certificate::decode(&hex(SUB_AGENT_WITH_STATEMENT)).unwrap();
    sub.statement.as_mut().unwrap().number = 2;
    assert!(!sub.verify(&creator().public().signing));
}

#[test]
fn key_6_that_is_not_a_statement_does_not_decode() {
    assert!(Certificate::decode(&with_key_6(CERTIFICATE, Value::Bytes(hex(STATEMENT)))).is_err());
    assert!(Certificate::decode(&with_key_6(CERTIFICATE, Value::Unsigned(1))).is_err());
    let Value::Map(mut map) = cbor::decode(&hex(STATEMENT)).unwrap() else {
        panic!("a statement is a map");
    };
    map.remove(&Key::Unsigned(4));
    assert!(Certificate::decode(&with_key_6(CERTIFICATE, Value::Map(map))).is_err());
}

#[test]
fn a_certificate_with_a_key_after_6_does_not_decode() {
    // Keys 4 to 63 are must-understand, and RFC 0003 defines none after 6.
    let Value::Map(mut map) = cbor::decode(&hex(WITH_STATEMENT)).unwrap() else {
        panic!("a certificate is a map");
    };
    map.insert(Key::Unsigned(7), Value::Unsigned(1));
    assert!(Certificate::decode(&cbor::encode(&Value::Map(map))).is_err());
}
