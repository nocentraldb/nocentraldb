//! The key chain entry of RFC 0008 ("Key chain entry", entry type 3): a
//! rotation's new generation id, each previous generation's key encrypted
//! under the new generation, and the revocations the rotation answers.
//!
//! ```text
//! 8  generation   uint, the new generation id
//! 9  previous     array of {1: generation id, 2: encrypted key}, sorted by
//!                 generation id, at least one
//! 10 answers      array of change ids, sorted by bytes
//! ```
//!
//! An encrypted key is 72 bytes, as the owner decided in NOC-97 (Carsten,
//! 2026-11-10): the 24-byte nonce, then the ciphertext of the 32-byte key
//! and its 16-byte tag. It is XChaCha20-Poly1305 with a random nonce under
//! RFC 0003's key-chain subkey of the new generation's key, with the space
//! id, the new generation id and that previous generation id as associated
//! data ([`seal_previous_key`]). Whether it opens only a holder of the new
//! key can tell, a local check (RFC 0008, "What the fold may read"), so the
//! fold checks only its length.

use std::collections::BTreeSet;
use std::fmt;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload as Aad};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::cbor::{Key, Value};
use crate::change::ChangeError;
use crate::ids::{ChangeId, SpaceId};
use crate::keys::{self, SpaceKey};
use crate::seal::NONCE_LEN;

use super::fields::{self, Fields};

/// The length of an encrypted previous key: nonce, ciphertext and tag.
pub const PREVIOUS_KEY_LEN: usize = 72;

/// The label of RFC 0003's key-chain subkey.
const KEY_CHAIN_CONTEXT: &[u8] = b"nocentraldb/key-chain";

/// The body keys of entry type 3.
const GENERATION: u64 = 8;
const PREVIOUS: u64 = 9;
const ANSWERS: u64 = 10;

/// A key chain entry (entry type 3). Its `Debug` output shows generation
/// ids and change ids, and each encrypted key only by its length.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyChainEntry {
    /// Key 8: the new generation id.
    pub generation: u64,
    /// Key 9: the previous generations, sorted by generation id.
    pub previous: Vec<PreviousKey>,
    /// Key 10: the revocations this rotation answers.
    pub answers: BTreeSet<ChangeId>,
}

/// One previous generation of a key chain entry.
#[derive(Clone, PartialEq, Eq)]
pub struct PreviousKey {
    /// Key 1: its generation id.
    pub generation: u64,
    /// Key 2: its key, encrypted under the new generation's key-chain
    /// subkey (RFC 0003) in NOC-97's layout ([`seal_previous_key`]).
    pub key: [u8; PREVIOUS_KEY_LEN],
}

impl KeyChainEntry {
    /// The ids of the previous generations, in order.
    pub fn previous_generations(&self) -> impl Iterator<Item = u64> + '_ {
        self.previous.iter().map(|p| p.generation)
    }

    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        let previous = self.previous.iter().map(|p| {
            fields::map([
                (1, Value::Unsigned(p.generation)),
                (2, fields::bytes(&p.key)),
            ])
        });
        let answers = self.answers.iter().map(|id| fields::bytes(&id.0));
        vec![
            (GENERATION, Value::Unsigned(self.generation)),
            (PREVIOUS, Value::Array(previous.collect())),
            (ANSWERS, Value::Array(answers.collect())),
        ]
    }

    /// Takes keys 8 to 10 out of the map. Every key is required; the
    /// previous generations are sorted by id, at least one and none twice,
    /// each a map of exactly keys 1 and 2, so each entry has one encoding.
    pub(crate) fn take(map: &mut Fields) -> Result<KeyChainEntry, ChangeError> {
        let generation = fields::unsigned(map.required(GENERATION)?)?;
        let previous = previous(map.required(PREVIOUS)?.1)?;
        let answers = fields::change_ids(map.required(ANSWERS)?)?;
        Ok(KeyChainEntry {
            generation,
            previous,
            answers,
        })
    }
}

fn previous(value: Value) -> Result<Vec<PreviousKey>, ChangeError> {
    let invalid = ChangeError::InvalidValue(PREVIOUS);
    let Value::Array(items) = value else {
        return Err(invalid);
    };
    let mut previous: Vec<PreviousKey> = Vec::with_capacity(items.len());
    for item in items {
        let Value::Map(entries) = item else {
            return Err(invalid);
        };
        let (Some(Value::Unsigned(generation)), Some(Value::Bytes(key)), 2) = (
            entries.get(&Key::Unsigned(1)),
            entries.get(&Key::Unsigned(2)),
            entries.len(),
        ) else {
            return Err(invalid);
        };
        if previous.last().is_some_and(|p| p.generation >= *generation) {
            return Err(invalid);
        }
        let Ok(key) = key.as_slice().try_into() else {
            return Err(invalid);
        };
        previous.push(PreviousKey {
            generation: *generation,
            key,
        });
    }
    if previous.is_empty() {
        return Err(invalid);
    }
    Ok(previous)
}

/// Encrypts the key of generation `previous` for the key chain entry of
/// generation `generation`, under `new_key`, the new generation's key, with
/// a random nonce: NOC-97's 72 bytes.
pub fn seal_previous_key(
    new_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    previous: u64,
    previous_key: &SpaceKey,
) -> [u8; PREVIOUS_KEY_LEN] {
    let nonce = keys::random::<NONCE_LEN>();
    seal_previous_key_with_nonce(new_key, space, generation, previous, previous_key, *nonce)
}

/// [`seal_previous_key`] with a given nonce, for test vectors. A nonce must
/// never be used twice under one key.
pub fn seal_previous_key_with_nonce(
    new_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    previous: u64,
    previous_key: &SpaceKey,
    nonce: [u8; NONCE_LEN],
) -> [u8; PREVIOUS_KEY_LEN] {
    let ciphertext = cipher(new_key, space, generation)
        .encrypt(
            &nonce.into(),
            Aad {
                msg: previous_key.bytes(),
                aad: &associated_data(space, generation, previous),
            },
        )
        .expect("32 bytes are far below XChaCha20-Poly1305's length limit");
    [nonce.as_slice(), &ciphertext]
        .concat()
        .try_into()
        .expect("the nonce, 32 bytes and the tag")
}

/// Opens an encrypted previous key; `None` when it does not decrypt under
/// this key, space and pair of generations. That is a local check (RFC
/// 0008, "What the fold may read"): the fold never calls it.
pub fn open_previous_key(
    new_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    previous: u64,
    field: &[u8; PREVIOUS_KEY_LEN],
) -> Option<SpaceKey> {
    let (nonce, ciphertext) = field.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce length");
    let plaintext = Zeroizing::new(
        cipher(new_key, space, generation)
            .decrypt(
                &nonce.into(),
                Aad {
                    msg: ciphertext,
                    aad: &associated_data(space, generation, previous),
                },
            )
            .ok()?,
    );
    let bytes: [u8; 32] = plaintext.as_slice().try_into().ok()?;
    Some(SpaceKey::from_bytes(bytes))
}

/// RFC 0003's key-chain subkey: HKDF-SHA256 of the new generation's key,
/// without salt, with the info `"nocentraldb/key-chain" || space id || new
/// generation id (8 bytes, big-endian)`.
fn cipher(new_key: &SpaceKey, space: SpaceId, generation: u64) -> XChaCha20Poly1305 {
    let info = [KEY_CHAIN_CONTEXT, &space.0, &generation.to_be_bytes()].concat();
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, new_key.bytes())
        .expand(&info, key.as_mut())
        .expect("32 bytes are far below HKDF-SHA256's output limit");
    XChaCha20Poly1305::new(&(*key).into())
}

/// The space id, the new generation id and the previous generation id, each
/// id 8 bytes, big-endian (RFC 0003, "Key-chain subkey").
fn associated_data(space: SpaceId, generation: u64, previous: u64) -> Vec<u8> {
    [
        space.0.as_slice(),
        &generation.to_be_bytes(),
        &previous.to_be_bytes(),
    ]
    .concat()
}

impl fmt::Debug for KeyChainEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyChainEntry")
            .field("generation", &self.generation)
            .field("previous", &self.previous)
            .field("answers", &self.answers)
            .finish()
    }
}

impl fmt::Debug for PreviousKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreviousKey")
            .field("generation", &self.generation)
            .field("key_len", &self.key.len())
            .finish()
    }
}
