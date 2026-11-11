//! The account key sealed in a pairing answer (RFC 0003, "Pairing answer"):
//! HPKE as for a key envelope, under the `info` label
//! `"nocentraldb/account-key"` alone. tests/vectors/key_envelope.py writes
//! the vector with its own RFC 9180 code, checked against RFC 9180's vector
//! A.2.1, and opens it with cryptography's HPKE.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::envelope::EnvelopeError;
use ncdb_core::keys::MemberKey;
use ncdb_core::pairing::{self, SealedAccountKey};

const INFO: &str = include_str!("vectors/pairing.info.hex");
const ENC: &str = include_str!("vectors/pairing.enc.hex");
const CIPHERTEXT: &str = include_str!("vectors/pairing.ciphertext.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The key envelope vector's recipient.
fn recipient() -> MemberKey {
    MemberKey::from_secrets(
        std::array::from_fn(|i| 0x10 + i as u8),
        std::array::from_fn(|i| 0x60 + i as u8),
    )
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn ikm() -> [u8; 32] {
    std::array::from_fn(|i| 0xb0 + i as u8)
}

fn vector() -> SealedAccountKey {
    SealedAccountKey {
        encapsulated_key: hex(ENC),
        ciphertext: hex(CIPHERTEXT),
    }
}

#[test]
fn the_info_is_the_label_alone() {
    assert_eq!(pairing::ACCOUNT_KEY_CONTEXT, b"nocentraldb/account-key");
    assert_eq!(pairing::ACCOUNT_KEY_CONTEXT.to_vec(), hex(INFO));
}

#[test]
fn sealing_with_the_vectors_ephemeral_gives_its_bytes() {
    let sealed = pairing::seal_account_key_with_ephemeral(&account(), &recipient().public(), ikm());
    assert_eq!(sealed, Ok(vector()));
}

#[test]
fn the_vector_opens_to_the_account_key() {
    let opened = pairing::open_account_key(&vector(), &recipient()).unwrap();
    assert_eq!(opened.to_bytes(), account().to_bytes());
}

#[test]
fn a_sealed_account_key_opens_only_for_its_recipient() {
    let sealed = pairing::seal_account_key(&account(), &recipient().public()).unwrap();
    assert_eq!(
        pairing::open_account_key(&sealed, &recipient()).map(|key| key.to_bytes()),
        Ok(account().to_bytes())
    );
    let other = MemberKey::from_secrets([1; 32], [2; 32]);
    assert_eq!(
        pairing::open_account_key(&sealed, &other).map(|key| key.to_bytes()),
        Err(EnvelopeError::Open)
    );
}

#[test]
fn a_sealed_account_key_does_not_open_as_a_key_envelope() {
    // The label keeps an account key from ever being opened as a space
    // key: the same HPKE output under the key envelope's info fails.
    let sealed = vector();
    let as_envelope = ncdb_core::envelope::KeyEnvelope {
        recipient: recipient().id(),
        generation: 1 << 48,
        encapsulated_key: sealed.encapsulated_key,
        ciphertext: sealed.ciphertext,
    };
    let space = ncdb_core::ids::SpaceId([0; 16]);
    assert_eq!(
        ncdb_core::envelope::open(&as_envelope, space, &recipient()).map(|_| ()),
        Err(EnvelopeError::Open)
    );
}

#[test]
fn a_changed_ciphertext_does_not_open() {
    let mut sealed = vector();
    sealed.ciphertext[0] ^= 1;
    assert_eq!(
        pairing::open_account_key(&sealed, &recipient()).map(|_| ()),
        Err(EnvelopeError::Open)
    );
}
