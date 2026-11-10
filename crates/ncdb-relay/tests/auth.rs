//! RFC 0007, "Opening a connection": the member signs
//! H("nocentraldb/relay-auth" || relay id || nonce) with its member key,
//! and the connection acts as the member id of the keys whose signature
//! verifies.

mod support;

use ed25519_dalek::{Signer, SigningKey};
use ncdb_relay::{ErrorCode, auth};
use sha2::{Digest, Sha256};
use support::space::{id, key};

const RELAY: &[u8] = b"relay-one";
const NONCE: [u8; 32] = [0x4e; 32];

/// The simulated member's signature over the challenge: its signing key is
/// the Ed25519 seed `[who; 32]`.
fn sign(who: u8, relay: &[u8], nonce: &[u8; 32]) -> Vec<u8> {
    let signing = SigningKey::from_bytes(&[who; 32]);
    signing
        .sign(&auth::digest(relay, nonce))
        .to_bytes()
        .to_vec()
}

#[test]
fn the_digest_hashes_the_context_the_relay_id_and_the_nonce() {
    let mut hash = Sha256::new();
    hash.update(b"nocentraldb/relay-auth");
    hash.update(RELAY);
    hash.update(NONCE);
    let expected: [u8; 32] = hash.finalize().into();
    assert_eq!(auth::digest(RELAY, &NONCE), expected);
    assert_eq!(auth::CONTEXT, b"nocentraldb/relay-auth");
}

#[test]
fn a_signed_challenge_names_the_member() {
    let signature = sign(3, RELAY, &NONCE);
    let verified = auth::verify(RELAY, &NONCE, &key(3).public(), &signature);
    assert_eq!(verified, Ok(id(3)));
}

#[test]
fn a_signature_over_another_relay_or_nonce_or_by_another_key_is_refused() {
    let other_relay = sign(3, b"relay-two", &NONCE);
    let other_nonce = sign(3, RELAY, &[0; 32]);
    let other_key = sign(4, RELAY, &NONCE);
    for signature in [other_relay, other_nonce, other_key, vec![0; 64], vec![1; 3]] {
        let verified = auth::verify(RELAY, &NONCE, &key(3).public(), &signature);
        assert_eq!(verified, Err(ErrorCode::Unauthenticated));
    }
}
