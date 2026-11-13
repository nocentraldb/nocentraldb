//! The link statement (RFC 0010, "The link statement"; Carsten,
//! 2026-11-12, NOC-273): a per-database account's old key and the account
//! key that a statement names both sign H("nocentraldb/account-link" ||
//! canonical map {1: old account key, 2: recovery key}). The vector is
//! docs/rfc/vectors/0010/pairing_answer.py's.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::{Signer, SigningKey};
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::pairing::{self, LinkStatement};

const VECTOR: &str = include_str!("../../../docs/rfc/vectors/0010/link-statement.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x42; 16])
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn old() -> SigningKey {
    SigningKey::from_bytes(&[0x33; 32])
}

fn statement() -> AccountKeyStatement {
    AccountKeyStatement::sign(&recovery(), account().verifying_key().to_bytes(), 2)
}

/// The account key's half, as a pairing answer carries it.
fn account_half() -> [u8; 64] {
    let digest = pairing::link_digest(&old().verifying_key().to_bytes(), &recovery().public());
    account().sign(&digest).to_bytes()
}

#[test]
fn the_old_key_completes_the_statement_and_it_is_the_vectors() {
    let link = LinkStatement::sign(&old(), &statement(), account_half());
    assert!(link.verify());
    assert_eq!(link.old_account_key, old().verifying_key().to_bytes());
    assert_eq!(link.recovery_key, recovery().public());
    assert_eq!(link.encode(), hex(VECTOR));
    assert_eq!(LinkStatement::decode(&hex(VECTOR)).unwrap(), link);
}

#[test]
fn a_statement_with_a_wrong_half_or_another_recovery_key_does_not_verify() {
    let good = LinkStatement::sign(&old(), &statement(), account_half());
    let mut old_half = good.clone();
    old_half.old_signature[0] ^= 1;
    let mut account_half = good.clone();
    account_half.account_signature[0] ^= 1;
    let mut recovery_key = good.clone();
    recovery_key.recovery_key[0] ^= 1;
    let mut another_statement = good.clone();
    another_statement.statement = AccountKeyStatement::sign(
        &RecoveryKey::from_code(&[0x43; 16]),
        account().verifying_key().to_bytes(),
        2,
    );
    for (bad, why) in [
        (old_half, "the old key's half"),
        (account_half, "the account key's half"),
        (recovery_key, "the recovery key"),
        (another_statement, "a statement of another recovery key"),
    ] {
        assert!(!bad.verify(), "{why}");
    }
}

#[test]
fn bytes_that_are_no_link_statement_are_refused() {
    let bytes = hex(VECTOR);
    assert!(LinkStatement::decode(&bytes[..bytes.len() - 1]).is_err());
    assert!(LinkStatement::decode(&[0xa0]).is_err());
}
