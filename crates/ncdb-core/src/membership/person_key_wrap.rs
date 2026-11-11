//! A person key wrapped under a generation of the space key, the wrapped
//! bytes of a person-key record (RFC 0003, "Person-key record"), as the
//! owner decided on NOC-209 (Carsten, 2026-11-11):
//!
//! ```text
//! person-key subkey = HKDF-SHA256(ikm = the generation's space key, salt = none,
//!                       info = "nocentraldb/person-key-wrap" || space id || generation id)
//! associated data   = space id || generation id (8 bytes, big-endian) || person-key id
//! wrapped bytes     = nonce (24) || ciphertext (32) || tag (16), XChaCha20-Poly1305
//! ```
//!
//! The record's writer signs the wrapped bytes ([`super::sign_record`]);
//! the wrap itself binds the space, the generation and the person key.

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::ids::SpaceId;
use crate::keys::{self, PersonKey, SpaceKey};
use crate::seal::NONCE_LEN;

/// The length of a wrapped person key: nonce, ciphertext and tag.
pub const WRAPPED_PERSON_KEY_LEN: usize = 72;

/// The label of RFC 0003's person-key subkey.
const WRAP_CONTEXT: &[u8] = b"nocentraldb/person-key-wrap";

/// Wraps `key`, the person key `person_key` of `space`, under `space_key`,
/// the key of `generation`, with a random nonce.
pub fn wrap_person_key(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    person_key: [u8; 16],
    key: &PersonKey,
) -> [u8; WRAPPED_PERSON_KEY_LEN] {
    let nonce = keys::random::<NONCE_LEN>();
    wrap_person_key_with_nonce(space_key, space, generation, person_key, key, *nonce)
}

/// [`wrap_person_key`] with a given nonce, for test vectors. A nonce must
/// never be used twice under one key.
pub fn wrap_person_key_with_nonce(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    person_key: [u8; 16],
    key: &PersonKey,
    nonce: [u8; NONCE_LEN],
) -> [u8; WRAPPED_PERSON_KEY_LEN] {
    let ciphertext = cipher(space_key, space, generation)
        .encrypt(
            &nonce.into(),
            Payload {
                msg: key.bytes(),
                aad: &associated_data(space, generation, person_key),
            },
        )
        .expect("32 bytes are far below XChaCha20-Poly1305's length limit");
    [nonce.as_slice(), &ciphertext]
        .concat()
        .try_into()
        .expect("the nonce, 32 bytes and the tag")
}

/// Unwraps the person key `person_key` of `space` from `wrapped`, under
/// `space_key`, the key of `generation`; `None` when it is not 72 bytes or
/// does not open under this key, space, generation and person-key id. A
/// member then also checks the key against the commitment in the log
/// ([`PersonKey::commitment`]).
pub fn unwrap_person_key(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    person_key: [u8; 16],
    wrapped: &[u8],
) -> Option<PersonKey> {
    if wrapped.len() != WRAPPED_PERSON_KEY_LEN {
        return None;
    }
    let (nonce, ciphertext) = wrapped.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce length");
    let plaintext = Zeroizing::new(
        cipher(space_key, space, generation)
            .decrypt(
                &nonce.into(),
                Payload {
                    msg: ciphertext,
                    aad: &associated_data(space, generation, person_key),
                },
            )
            .ok()?,
    );
    let bytes: [u8; 32] = plaintext.as_slice().try_into().ok()?;
    Some(PersonKey::from_bytes(bytes))
}

/// RFC 0003's person-key subkey: HKDF-SHA256 of the generation's key,
/// without salt, with the info `"nocentraldb/person-key-wrap" || space id
/// || generation id (8 bytes, big-endian)`.
fn cipher(space_key: &SpaceKey, space: SpaceId, generation: u64) -> XChaCha20Poly1305 {
    let info = [WRAP_CONTEXT, &space.0, &generation.to_be_bytes()].concat();
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, space_key.bytes())
        .expand(&info, key.as_mut())
        .expect("32 bytes are far below HKDF-SHA256's output limit");
    XChaCha20Poly1305::new(&(*key).into())
}

/// The space id, the generation id (8 bytes, big-endian) and the
/// person-key id, 40 bytes (RFC 0003, "Person-key record").
fn associated_data(space: SpaceId, generation: u64, person_key: [u8; 16]) -> Vec<u8> {
    [space.0.as_slice(), &generation.to_be_bytes(), &person_key].concat()
}
