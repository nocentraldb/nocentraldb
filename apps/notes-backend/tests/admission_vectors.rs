//! The backend's admissions are RFC 0015's format: signed with the vectors'
//! inputs, they are byte for byte the committed vectors of
//! `docs/rfc/vectors/0015`, which `generate.py` writes with Python.

use ed25519_dalek::SigningKey;
use notes_backend::admission::{Grant, identity, sign};

const FIELDS: &str = include_str!("../../../docs/rfc/vectors/0015/fields.txt");
const RECOVERY_KEY: &str = include_str!("../../../docs/rfc/vectors/0015/admission.txt");
const ACCOUNT_KEY: &str = include_str!("../../../docs/rfc/vectors/0015/admission-account-key.txt");

/// The field `name` of fields.txt, as hex.
fn field(name: &str) -> &'static str {
    FIELDS
        .lines()
        .find_map(|line| line.strip_prefix(name)?.strip_prefix(": "))
        .unwrap_or_else(|| panic!("fields.txt names {name}"))
}

/// The vectors' grant for the identity in the field `identity_field`.
fn grant(identity_field: &str, spaces: u64) -> Grant {
    let relay_id: Vec<u8> = (0..16).collect();
    Grant {
        relay_id: relay_id.try_into().unwrap(),
        identity: identity(field(identity_field)).expect("64 hex digits"),
        until: u64::from_str_radix(field("until"), 16).unwrap(),
        spaces,
    }
}

/// The vectors' admission key: the secret is 32 bytes 0x5a.
fn key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

#[test]
fn an_admission_for_a_recovery_key_is_the_committed_vector() {
    assert_eq!(
        sign(&key(), &grant("recovery key", 20)),
        RECOVERY_KEY.trim()
    );
}

#[test]
fn an_admission_for_an_account_key_is_the_committed_vector() {
    assert_eq!(sign(&key(), &grant("account key", 1)), ACCOUNT_KEY.trim());
}

#[test]
fn an_identity_is_64_lowercase_hex_digits() {
    assert_eq!(identity(&"ab".repeat(32)), Some([0xab; 32]));
    assert_eq!(identity(&"AB".repeat(32)), None);
    assert_eq!(identity("ab"), None);
    assert_eq!(identity(&"zz".repeat(32)), None);
}
