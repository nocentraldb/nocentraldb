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
use std::convert::Infallible;
use std::fmt;

use hpke::rand_core::{TryCryptoRng, TryRng};
use hpke::{Deserializable, Kem, OpModeR, OpModeS, Serializable};
use zeroize::Zeroizing;

use crate::cbor::{Key, Value};
use crate::ids::{MemberId, SpaceId};
use crate::keys::{self, MemberKey, MemberPublicKey, SpaceKey};

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
    /// The recipient's X25519 public key is not one HPKE can encapsulate
    /// to, such as a point of small order.
    RecipientKey,
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
            EnvelopeError::RecipientKey => {
                f.write_str("the recipient's agreement key does not take a key envelope")
            }
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
    let mut info = Vec::with_capacity(ENVELOPE_CONTEXT.len() + 16 + 8 + 32);
    info.extend_from_slice(ENVELOPE_CONTEXT);
    info.extend_from_slice(&space.0);
    info.extend_from_slice(&generation.to_be_bytes());
    info.extend_from_slice(&recipient.0);
    info
}

/// Wraps a generation of a space key to the recipient's X25519 key. The
/// ephemeral key comes from the platform's secure generator.
///
/// # Panics
///
/// When the platform has no secure generator.
pub fn seal(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    recipient: &MemberPublicKey,
) -> Result<KeyEnvelope, EnvelopeError> {
    let ikm = keys::random::<32>();
    seal_with_ephemeral(space_key, space, generation, recipient, *ikm)
}

/// [`seal`] with the ephemeral key derived from `ikm` (RFC 9180's ikmE),
/// for test vectors. An ephemeral key must never be used twice; outside
/// test vectors, use [`seal`].
pub fn seal_with_ephemeral(
    space_key: &SpaceKey,
    space: SpaceId,
    generation: u64,
    recipient: &MemberPublicKey,
    ikm: [u8; 32],
) -> Result<KeyEnvelope, EnvelopeError> {
    let (encapsulated_key, ciphertext) = seal_bytes(
        space_key.bytes(),
        &info(space, generation, recipient.id()),
        recipient,
        ikm,
    )?;
    Ok(KeyEnvelope {
        recipient: recipient.id(),
        generation,
        encapsulated_key,
        ciphertext,
    })
}

/// Seals 32 secret bytes with RFC 0003's HPKE suite, base mode, under
/// `info` and no associated data, to the recipient's X25519 key, with the
/// ephemeral key derived from `ikm`: HPKE's encapsulated key and the
/// ciphertext. Key envelopes and the account key of a pairing answer
/// ([`crate::pairing`]) share it and differ only by `info`.
pub(crate) fn seal_bytes(
    plaintext: &[u8; 32],
    info: &[u8],
    recipient: &MemberPublicKey,
    ikm: [u8; 32],
) -> Result<(Vec<u8>, Vec<u8>), EnvelopeError> {
    let public = <suite::Kem as Kem>::PublicKey::from_bytes(&recipient.agreement)
        .map_err(|_| EnvelopeError::RecipientKey)?;
    let (encapsulated_key, ciphertext) =
        hpke::single_shot_seal_with_rng::<suite::Aead, suite::Kdf, suite::Kem>(
            &OpModeS::Base,
            &public,
            info,
            plaintext,
            &[],
            &mut Ephemeral(Some(Zeroizing::new(ikm))),
        )
        .map_err(|_| EnvelopeError::RecipientKey)?;
    Ok((encapsulated_key.to_bytes().to_vec(), ciphertext))
}

/// Opens what [`seal_bytes`] sealed to this member under `info`: the 32
/// secret bytes, wiped when dropped.
pub(crate) fn open_bytes(
    encapsulated_key: &[u8],
    ciphertext: &[u8],
    info: &[u8],
    recipient: &MemberKey,
) -> Result<Zeroizing<[u8; 32]>, EnvelopeError> {
    let private = <suite::Kem as Kem>::PrivateKey::from_bytes(recipient.agreement())
        .map_err(|_| EnvelopeError::Open)?;
    let encapsulated_key = <suite::Kem as Kem>::EncappedKey::from_bytes(encapsulated_key)
        .map_err(|_| EnvelopeError::Open)?;
    let plaintext = Zeroizing::new(
        hpke::single_shot_open::<suite::Aead, suite::Kdf, suite::Kem>(
            &OpModeR::Base,
            &private,
            &encapsulated_key,
            info,
            ciphertext,
            &[],
        )
        .map_err(|_| EnvelopeError::Open)?,
    );
    let bytes: &[u8; 32] = plaintext
        .as_slice()
        .try_into()
        .map_err(|_| EnvelopeError::Open)?;
    Ok(Zeroizing::new(*bytes))
}

/// Unwraps the space key of an envelope in this space for this member.
pub fn open(
    envelope: &KeyEnvelope,
    space: SpaceId,
    recipient: &MemberKey,
) -> Result<SpaceKey, EnvelopeError> {
    if envelope.recipient != recipient.id() {
        return Err(EnvelopeError::WrongRecipient);
    }
    let bytes = open_bytes(
        &envelope.encapsulated_key,
        &envelope.ciphertext,
        &info(space, envelope.generation, envelope.recipient),
        recipient,
    )?;
    Ok(SpaceKey::from_bytes(*bytes))
}

impl KeyEnvelope {
    /// Keys 8 to 11 of the control body.
    pub fn fields(&self) -> BTreeMap<Key, Value> {
        BTreeMap::from([
            (
                Key::Unsigned(RECIPIENT),
                Value::Bytes(self.recipient.0.to_vec()),
            ),
            (Key::Unsigned(GENERATION), Value::Unsigned(self.generation)),
            (
                Key::Unsigned(ENCAPSULATED_KEY),
                Value::Bytes(self.encapsulated_key.clone()),
            ),
            (
                Key::Unsigned(CIPHERTEXT),
                Value::Bytes(self.ciphertext.clone()),
            ),
        ])
    }

    /// The envelope of keys 8 to 11 of a control body; other keys are not
    /// the envelope's and are left to the control body. Only the recipient
    /// has a fixed length here: whether the encapsulated key and the
    /// ciphertext open is [`open`]'s to tell.
    pub fn from_fields(fields: &BTreeMap<Key, Value>) -> Result<KeyEnvelope, EnvelopeError> {
        let bytes = |key| match fields.get(&Key::Unsigned(key)) {
            Some(Value::Bytes(bytes)) => Ok(bytes),
            _ => Err(EnvelopeError::Malformed(key)),
        };
        let recipient = bytes(RECIPIENT)?
            .as_slice()
            .try_into()
            .map_err(|_| EnvelopeError::Malformed(RECIPIENT))?;
        let Some(Value::Unsigned(generation)) = fields.get(&Key::Unsigned(GENERATION)) else {
            return Err(EnvelopeError::Malformed(GENERATION));
        };
        Ok(KeyEnvelope {
            recipient: MemberId(recipient),
            generation: *generation,
            encapsulated_key: bytes(ENCAPSULATED_KEY)?.clone(),
            ciphertext: bytes(CIPHERTEXT)?.clone(),
        })
    }
}

// The body keys of entry type 2 (RFC 0008, "Key envelope").
const RECIPIENT: u64 = 8;
const GENERATION: u64 = 9;
const ENCAPSULATED_KEY: u64 = 10;
const CIPHERTEXT: u64 = 11;

/// Hands HPKE the input of its ephemeral key, once: HPKE derives the
/// ephemeral key pair from it (RFC 9180, DeriveKeyPair), as its test vectors
/// do with ikmE. The input is wiped when HPKE has taken it.
struct Ephemeral(Option<Zeroizing<[u8; 32]>>);

impl TryRng for Ephemeral {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        unreachable!("HPKE draws the ephemeral key's input with fill_bytes")
    }

    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        unreachable!("HPKE draws the ephemeral key's input with fill_bytes")
    }

    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        let ikm = self
            .0
            .take()
            .expect("HPKE draws one ephemeral key per seal");
        dst.copy_from_slice(ikm.as_slice());
        Ok(())
    }
}

impl TryCryptoRng for Ephemeral {}
