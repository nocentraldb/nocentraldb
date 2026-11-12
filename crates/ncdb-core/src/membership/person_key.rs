//! The person-key entry (entry type 4) and the shred entry (entry type 5)
//! of RFC 0008, with the account sign-off of RFC 0003 ("Account sign-off of
//! a shred") and the signed person-key record of RFC 0007 ("Keys and the
//! person-key store").
//!
//! ```text
//! person-key entry   8 person-key id (16 bytes)   9 commitment (32 bytes)
//!                    10 generation (uint)          11 record hash (32 bytes)
//! shred entry        8 person-key id (16 bytes)   9 sign-off {1: account key, 2: signature}
//! ```
//!
//! The account signs `H("nocentraldb/shred-sign-off" || space id ||
//! person-key id || performer member id)`. A record's writer signs
//! `H("nocentraldb/person-key-record" || space id || person-key id ||
//! generation (8 bytes, big-endian) || writer member id || wrapped bytes)`,
//! and the record hash in the entry is the SHA-256 of the wrapped bytes.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::cbor::Value;
use crate::change::ChangeError;
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey};
use crate::redact;

use super::fields::{self, Fields};

/// A person-key entry (entry type 4): a person key, and one record of it.
#[derive(Clone, PartialEq, Eq)]
pub struct PersonKeyEntry {
    /// Key 8: the person-key id, 16 random bytes.
    pub person_key: [u8; 16],
    /// Key 9: the commitment to the key (RFC 0003).
    pub commitment: [u8; 32],
    /// Key 10: the generation id the record is wrapped under.
    pub generation: u64,
    /// Key 11: the SHA-256 of the record's wrapped bytes ([`record_hash`]).
    pub record_hash: [u8; 32],
}

/// A shred entry (entry type 5): the person key it destroys, and the sign-off
/// of a second account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShredEntry {
    /// Key 8: the person-key id.
    pub person_key: [u8; 16],
    /// Key 9: the account sign-off.
    pub sign_off: SignOff,
}

/// The account sign-off of a shred (RFC 0003): an account key and its
/// signature over the space, the person key and the performer.
#[derive(Clone, PartialEq, Eq)]
pub struct SignOff {
    /// Key 1: the account's Ed25519 public key.
    pub account_key: [u8; 32],
    /// Key 2: the signature.
    pub signature: [u8; 64],
}

/// The context of a sign-off's hash.
pub const SIGN_OFF_CONTEXT: &[u8] = b"nocentraldb/shred-sign-off";

/// The context of a person-key record's hash.
pub const RECORD_CONTEXT: &[u8] = b"nocentraldb/person-key-record";

impl SignOff {
    /// What the account signs.
    pub fn digest(space: SpaceId, person_key: [u8; 16], performer: MemberId) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(SIGN_OFF_CONTEXT);
        hash.update(space.0);
        hash.update(person_key);
        hash.update(performer.0);
        hash.finalize().into()
    }

    /// The sign-off of `account` for `performer` to shred `person_key`.
    pub fn sign(
        account: &SigningKey,
        space: SpaceId,
        person_key: [u8; 16],
        performer: MemberId,
    ) -> SignOff {
        let digest = SignOff::digest(space, person_key, performer);
        SignOff {
            account_key: account.verifying_key().to_bytes(),
            signature: account.sign(&digest).to_bytes(),
        }
    }

    /// Whether the signature verifies, strictly, with its account key for
    /// this space, person key and performer. Whether the account may sign
    /// off is the fold's to check.
    pub fn verify(&self, space: SpaceId, person_key: [u8; 16], performer: MemberId) -> bool {
        verify(
            &self.account_key,
            &SignOff::digest(space, person_key, performer),
            &self.signature,
        )
    }

    fn to_value(&self) -> Value {
        fields::map([
            (1, fields::bytes(&self.account_key)),
            (2, fields::bytes(&self.signature)),
        ])
    }

    /// The sign-off from its map, keys 1 and 2 and no other.
    fn from_value(value: Value) -> Option<SignOff> {
        let mut map = Fields::of(value).ok()?;
        let account_key = fields::fixed(map.required(1).ok()?).ok()?;
        let signature = fields::fixed(map.required(2).ok()?).ok()?;
        map.0.is_empty().then_some(SignOff {
            account_key,
            signature,
        })
    }
}

impl PersonKeyEntry {
    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        vec![
            (8, fields::bytes(&self.person_key)),
            (9, fields::bytes(&self.commitment)),
            (10, Value::Unsigned(self.generation)),
            (11, fields::bytes(&self.record_hash)),
        ]
    }

    pub(crate) fn take(map: &mut Fields) -> Result<PersonKeyEntry, ChangeError> {
        Ok(PersonKeyEntry {
            person_key: fields::fixed(map.required(8)?)?,
            commitment: fields::fixed(map.required(9)?)?,
            generation: fields::unsigned(map.required(10)?)?,
            record_hash: fields::fixed(map.required(11)?)?,
        })
    }
}

/// Entry type 8: the decision that ends a provisional erasure, either way
/// (RFC 0008, "Erasure decision"; Carsten, 2026-11-11, NOC-230).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErasureDecision {
    /// Key 8: the change id of the provisional shred entry.
    pub shred: ChangeId,
    /// Key 9: true finalizes the erasure, false takes it back.
    pub finalize: bool,
}

impl ErasureDecision {
    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        vec![
            (8, fields::bytes(&self.shred.0)),
            (9, Value::Bool(self.finalize)),
        ]
    }

    pub(crate) fn take(map: &mut Fields) -> Result<ErasureDecision, ChangeError> {
        let shred = ChangeId(fields::fixed(map.required(8)?)?);
        let finalize = match map.required(9)?.1 {
            Value::Bool(finalize) => finalize,
            _ => return Err(ChangeError::InvalidValue(9)),
        };
        Ok(ErasureDecision { shred, finalize })
    }
}

impl ShredEntry {
    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        vec![
            (8, fields::bytes(&self.person_key)),
            (9, self.sign_off.to_value()),
        ]
    }

    pub(crate) fn take(map: &mut Fields) -> Result<ShredEntry, ChangeError> {
        let person_key = fields::fixed(map.required(8)?)?;
        let sign_off =
            SignOff::from_value(map.required(9)?.1).ok_or(ChangeError::InvalidValue(9))?;
        Ok(ShredEntry {
            person_key,
            sign_off,
        })
    }
}

/// The context string of the commitment to a person key (RFC 0003, "The
/// person key").
const COMMIT_CONTEXT: &[u8] = b"nocentraldb/person-key-commit";

/// The commitment to a person key that every person-key entry carries
/// (RFC 0003, "The person key"): `SHA-256("nocentraldb/person-key-commit"
/// || space id || person-key id || key)`. The key is 32 random bytes, so
/// the commitment reveals nothing about it, before or after a shred; a
/// member that unwraps a record checks the key against it.
pub fn person_key_commitment(space: SpaceId, person_key: [u8; 16], key: &[u8; 32]) -> [u8; 32] {
    Sha256::new()
        .chain_update(COMMIT_CONTEXT)
        .chain_update(space.0)
        .chain_update(person_key)
        .chain_update(key)
        .finalize()
        .into()
}

/// The record hash a person-key entry announces: the SHA-256 of the
/// record's wrapped bytes.
pub fn record_hash(wrapped: &[u8]) -> [u8; 32] {
    Sha256::digest(wrapped).into()
}

/// What a record's writer signs.
fn record_digest(
    space: SpaceId,
    person_key: [u8; 16],
    generation: u64,
    writer: MemberId,
    wrapped: &[u8],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(RECORD_CONTEXT);
    hash.update(space.0);
    hash.update(person_key);
    hash.update(generation.to_be_bytes());
    hash.update(writer.0);
    hash.update(wrapped);
    hash.finalize().into()
}

/// The signature of `writer` over a person-key record.
pub fn sign_record(
    writer: &MemberKey,
    space: SpaceId,
    person_key: [u8; 16],
    generation: u64,
    wrapped: &[u8],
) -> [u8; 64] {
    let digest = record_digest(space, person_key, generation, writer.id(), wrapped);
    writer.signing().sign(&digest).to_bytes()
}

/// Whether `signature` is `writer`'s, strictly, over a person-key record.
pub fn verify_record(
    writer: &MemberPublicKey,
    space: SpaceId,
    person_key: [u8; 16],
    generation: u64,
    wrapped: &[u8],
    signature: &[u8],
) -> bool {
    let Ok(signature) = <[u8; 64]>::try_from(signature) else {
        return false;
    };
    let digest = record_digest(space, person_key, generation, writer.id(), wrapped);
    verify(&writer.signing, &digest, &signature)
}

fn verify(key: &[u8; 32], digest: &[u8; 32], signature: &[u8; 64]) -> bool {
    VerifyingKey::from_bytes(key).is_ok_and(|key| {
        key.verify_strict(digest, &Signature::from_bytes(signature))
            .is_ok()
    })
}

/// Shows ids and public keys as hex, never more.
struct Hex<'a>(&'a [u8]);

impl fmt::Debug for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        redact::hex(f, self.0)
    }
}

impl fmt::Debug for PersonKeyEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PersonKeyEntry")
            .field("person_key", &Hex(&self.person_key))
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for SignOff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignOff")
            .field("account_key", &Hex(&self.account_key))
            .finish_non_exhaustive()
    }
}
