//! The first change of a space (RFC 0008, "The first change of a space"):
//! the genesis body, the space id it derives, the generation ids of RFC
//! 0008's "Generation ids", and the sealed model id of its key 13 (decision
//! M26 B, RFC 0003's "Model-id subkey").

use std::fmt;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload as Aad};
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::cbor::{self, Value};
use crate::change::ChangeError;
use crate::ids::{MemberId, SpaceId};
use crate::keys::{self, MemberPublicKey, SpaceKey};
use crate::seal::NONCE_LEN;

use super::certificate::Certificate;
use super::fields::{self, Fields};

/// The context of the space id hash.
pub const SPACE_ID_CONTEXT: &[u8] = b"nocentraldb/space-id";
/// The context of the generation id hash.
pub const GENERATION_CONTEXT: &[u8] = b"nocentraldb/generation";
/// The label of the model-id subkey, and the start of its associated data.
pub const MODEL_ID_CONTEXT: &[u8] = b"nocentraldb/model-id";
/// The length of the sealed model id: nonce, ciphertext and tag.
pub const MODEL_ID_FIELD_LEN: usize = 56;

/// The body of a genesis (entry type 0). Its `Debug` output shows public
/// keys and ids, and the sealed model id only by its length.
#[derive(Clone, PartialEq, Eq)]
pub struct Genesis {
    /// Key 8: the 16 random bytes the space id derives from.
    pub nonce: [u8; 16],
    /// Keys 9 and 10: the creator's signing and agreement keys.
    pub creator: MemberPublicKey,
    /// Key 11: the creator's member certificate.
    pub certificate: Certificate,
    /// Key 12: the generation id of generation 1.
    pub first_generation: u64,
    /// Key 13: the creator's `model.id`, sealed for members.
    pub model_id: [u8; MODEL_ID_FIELD_LEN],
}

impl Genesis {
    /// The space id this genesis names: [`space_id`] of the creator and the
    /// nonce.
    pub fn space_id(&self) -> SpaceId {
        space_id(self.creator.id(), &self.nonce)
    }

    /// The generation id the first generation must have: [`generation_id`]
    /// with no previous generations.
    pub fn expected_first_generation(&self) -> u64 {
        generation_id(self.space_id(), self.creator.id(), &[])
    }

    pub(crate) fn fields(&self) -> Vec<(u64, Value)> {
        vec![
            (8, fields::bytes(&self.nonce)),
            (9, fields::bytes(&self.creator.signing)),
            (10, fields::bytes(&self.creator.agreement)),
            (11, self.certificate.to_value()),
            (12, Value::Unsigned(self.first_generation)),
            (13, fields::bytes(&self.model_id)),
        ]
    }

    pub(crate) fn take(map: &mut Fields) -> Result<Genesis, ChangeError> {
        Ok(Genesis {
            nonce: fields::fixed(map.required(8)?)?,
            creator: MemberPublicKey {
                signing: fields::fixed(map.required(9)?)?,
                agreement: fields::fixed(map.required(10)?)?,
            },
            certificate: Certificate::from_value(map.required(11)?.1)
                .map_err(|_| ChangeError::InvalidValue(11))?,
            first_generation: fields::unsigned(map.required(12)?)?,
            model_id: fields::fixed(map.required(13)?)?,
        })
    }
}

impl fmt::Debug for Genesis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Genesis")
            .field("space", &self.space_id())
            .field("creator", &self.creator)
            .field("certificate", &self.certificate)
            .field("first_generation", &self.first_generation)
            .finish_non_exhaustive()
    }
}

/// The space id of a creator and a nonce: the first 16 bytes of
/// `SHA-256("nocentraldb/space-id" || creator member id || nonce)`.
pub fn space_id(creator: MemberId, nonce: &[u8; 16]) -> SpaceId {
    let mut hash = Sha256::new();
    hash.update(SPACE_ID_CONTEXT);
    hash.update(creator.0);
    hash.update(nonce);
    let digest: [u8; 32] = hash.finalize().into();
    SpaceId(digest[..16].try_into().expect("16 of 32 bytes"))
}

/// A generation id: one more than the highest previous ordinal in the upper
/// 16 bits (1 with none), and below them the first 48 bits of
/// `SHA-256("nocentraldb/generation" || space id || author member id ||
/// canonical array of the previous generation ids)`. `previous` is taken
/// as given; RFC 0008 lists it sorted.
pub fn generation_id(space: SpaceId, author: MemberId, previous: &[u64]) -> u64 {
    let ordinal = previous.iter().map(|id| id >> 48).max().unwrap_or(0) + 1;
    let array = Value::Array(previous.iter().map(|id| Value::Unsigned(*id)).collect());
    let mut hash = Sha256::new();
    hash.update(GENERATION_CONTEXT);
    hash.update(space.0);
    hash.update(author.0);
    hash.update(cbor::encode(&array));
    let digest: [u8; 32] = hash.finalize().into();
    let mut tag = [0; 8];
    tag[2..].copy_from_slice(&digest[..6]);
    (ordinal << 48) | u64::from_be_bytes(tag)
}

/// Seals a `model.id` for a genesis, with a random nonce.
pub fn seal_model_id(
    generation_1: &SpaceKey,
    space: SpaceId,
    creator: MemberId,
    model_id: [u8; 16],
) -> [u8; MODEL_ID_FIELD_LEN] {
    let nonce = keys::random::<NONCE_LEN>();
    seal_model_id_with_nonce(generation_1, space, creator, model_id, *nonce)
}

/// [`seal_model_id`] with a given nonce, for test vectors.
pub fn seal_model_id_with_nonce(
    generation_1: &SpaceKey,
    space: SpaceId,
    creator: MemberId,
    model_id: [u8; 16],
    nonce: [u8; NONCE_LEN],
) -> [u8; MODEL_ID_FIELD_LEN] {
    let ciphertext = cipher(generation_1, space)
        .encrypt(
            &nonce.into(),
            Aad {
                msg: &model_id,
                aad: &associated_data(space, creator),
            },
        )
        .expect("16 bytes are far below XChaCha20-Poly1305's length limit");
    [nonce.as_slice(), &ciphertext]
        .concat()
        .try_into()
        .expect("nonce, 16 bytes and the tag")
}

/// Opens a genesis's sealed model id; `None` when it does not decrypt under
/// this key, space and creator. That is a local check (RFC 0008, "What the
/// fold may read"): the fold never calls it.
pub fn open_model_id(
    generation_1: &SpaceKey,
    space: SpaceId,
    creator: MemberId,
    field: &[u8; MODEL_ID_FIELD_LEN],
) -> Option<[u8; 16]> {
    let (nonce, ciphertext) = field.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce length");
    cipher(generation_1, space)
        .decrypt(
            &nonce.into(),
            Aad {
                msg: ciphertext,
                aad: &associated_data(space, creator),
            },
        )
        .ok()?
        .try_into()
        .ok()
}

/// The model-id subkey: HKDF-SHA256 of generation 1's key, without salt,
/// with the info `"nocentraldb/model-id" || space id`.
fn cipher(generation_1: &SpaceKey, space: SpaceId) -> XChaCha20Poly1305 {
    let info = [MODEL_ID_CONTEXT, &space.0].concat();
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, generation_1.bytes())
        .expand(&info, key.as_mut())
        .expect("32 bytes are far below HKDF-SHA256's output limit");
    XChaCha20Poly1305::new(&(*key).into())
}

/// `"nocentraldb/model-id" || space id || creator member id`.
fn associated_data(space: SpaceId, creator: MemberId) -> Vec<u8> {
    [MODEL_ID_CONTEXT, &space.0, &creator.0].concat()
}
