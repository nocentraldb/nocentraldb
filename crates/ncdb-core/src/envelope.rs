//! Key envelopes, as RFC 0003 defines them ("Key envelope"): one
//! generation's space key, wrapped with HPKE (RFC 9180) to one member's
//! X25519 key, so that only that member can unwrap it.
//!
//! The suite is RFC 0003's: HPKE base mode with DHKEM(X25519, HKDF-SHA256),
//! HKDF-SHA256 and ChaCha20-Poly1305. The HPKE `info` binds the envelope to
//! its space, its generation and its recipient: `"nocentraldb/key-envelope"
//! || space id (16 bytes) || generation id (8 bytes, big-endian) ||
//! recipient member id (32 bytes)`. The plaintext is the 32-byte space key.
//! RFC 0003 defines no associated data, so it is empty.
//!
//! RFC 0008 carries an envelope as the control body of entry type 2, with
//! the recipient (key 8), the generation (key 9), HPKE's encapsulated key
//! (key 10) and the ciphertext (key 11). [`KeyEnvelope::fields`] and
//! [`KeyEnvelope::from_fields`] write and read those four keys; the rest of
//! the control body, and the signature of the control change, are not
//! this module's.

use std::collections::BTreeMap;
use std::fmt;

use crate::cbor::{Key, Value};
use crate::ids::{MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey, SpaceKey};

/// The context string in front of the HPKE info.
pub const ENVELOPE_CONTEXT: &[u8] = b"nocentraldb/key-envelope";

/// The length of HPKE's encapsulated key: an X25519 public key.
pub const ENCAPSULATED_KEY_LEN: usize = 32;

/// The length of the ciphertext: the 32-byte space key and a 16-byte tag.
pub const CIPHERTEXT_LEN: usize = 48;

/// The HPKE suite of RFC 0003, as the `hpke` crate names its parts.
pub mod suite {
    /// DHKEM(X25519, HKDF-SHA256).
    pub type Kem = hpke::kem::X25519HkdfSha256;
    /// HKDF-SHA256.
    pub type Kdf = hpke::kdf::HkdfSha256;
    /// ChaCha20-Poly1305.
    pub type Aead = hpke::aead::ChaCha20Poly1305;
}

/// A key envelope: the fields of RFC 0008's entry type 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEnvelope {
    /// Key 8: the member id the envelope is for.
    pub recipient: MemberId,
    /// Key 9: the generation id of the wrapped space key.
    pub generation: u64,
    /// Key 10: HPKE's encapsulated key.
    pub encapsulated_key: Vec<u8>,
    /// Key 11: the HPKE ciphertext of the 32-byte space key.
    pub ciphertext: Vec<u8>,
}

/// Why an envelope does not read or does not open. No variant carries a
/// key or a ciphertext.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeError {
    /// The envelope is for another member.
    WrongRecipient,
    /// The body key is missing or does not hold what the envelope needs.
    Malformed(u64),
    /// HPKE does not open the envelope with this member's key, under the
    /// info of this space, generation and recipient.
    Open,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::WrongRecipient => f.write_str("the key envelope is for another member"),
            EnvelopeError::Malformed(key) => write!(f, "key envelope field {key} is malformed"),
            EnvelopeError::Open => f.write_str("the key envelope does not open"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// The HPKE info of an envelope for this recipient, of this generation of
/// this space's key.
pub fn info(space: SpaceId, generation: u64, recipient: MemberId) -> Vec<u8> {
    let _ = (space, generation, recipient);
    todo!()
}

/// Wraps a generation of a space key to the recipient's X25519 key. The
/// ephemeral key comes from the platform's secure generator.
pub fn seal(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    recipient: &MemberPublicKey,
) -> KeyEnvelope {
    let _ = (space_key, space, generation, recipient);
    todo!()
}

/// [`seal`] with the ephemeral key derived from `ikm` (RFC 9180's ikmE),
/// for test vectors. Outside test vectors, use [`seal`].
pub fn seal_with_ephemeral(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    recipient: &MemberPublicKey,
    ikm: [u8; 32],
) -> KeyEnvelope {
    let _ = (space_key, space, generation, recipient, ikm);
    todo!()
}

/// Unwraps the space key of an envelope in this space for this member.
pub fn open(
    envelope: &KeyEnvelope,
    space: SpaceId,
    recipient: &MemberKey,
) -> Result<SpaceKey, EnvelopeError> {
    let _ = (envelope, space, recipient);
    todo!()
}

impl KeyEnvelope {
    /// Keys 8 to 11 of the control body.
    pub fn fields(&self) -> BTreeMap<Key, Value> {
        todo!()
    }

    /// The envelope of keys 8 to 11 of a control body; other keys are not
    /// the envelope's and are left to the control body.
    pub fn from_fields(fields: &BTreeMap<Key, Value>) -> Result<KeyEnvelope, EnvelopeError> {
        let _ = fields;
        todo!()
    }
}
