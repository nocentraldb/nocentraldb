//! Signing and sealing a change, as RFC 0003 defines it ("How a member
//! signs and encrypts a change"), for suite 1.
//!
//! Sealing pads the payload map to a multiple of 256 bytes (RFC 0002, D8),
//! encrypts it with XChaCha20-Poly1305 under the content key of the space
//! key generation, with every header field except the payload as associated
//! data, and then signs the header bytes, ciphertext included, with the
//! author's Ed25519 key: encrypt-then-sign. Opening verifies the signature
//! with `verify_strict` and then decrypts.
//!
//! The content key is HKDF-SHA256 of the space key, without salt, with the
//! info `"nocentraldb/content-key" || space id (16 bytes) || key generation
//! (8 bytes, big-endian) || suite id (8 bytes, big-endian)`. The payload
//! bytes in header key 6 are the 24-byte nonce followed by the ciphertext
//! and its 16-byte tag. The associated data is the canonical encoding of the
//! header map without key 6.

use std::fmt;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload as Aad};
use ed25519_dalek::{Signature, Signer, VerifyingKey};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::change::{Change, ChangeError, Header, Kind, Payload};
use crate::ids::SpaceId;
use crate::keys::{self, MemberKey, MemberPublicKey, SpaceKey};

/// Suite 1 of RFC 0003: Ed25519, HPKE with X25519, XChaCha20-Poly1305,
/// HKDF-SHA256 and SHA-256.
pub const SUITE: u64 = 1;

/// Every sealed payload is padded to a multiple of this many bytes before
/// encryption (RFC 0002, D8).
pub const PADDING_BLOCK: usize = 256;

/// The length of the random nonce in front of the ciphertext.
pub const NONCE_LEN: usize = 24;

/// The length of the authentication tag behind the ciphertext.
pub const TAG_LEN: usize = 16;

/// The context string in front of the content key's HKDF info.
pub const CONTENT_KEY_CONTEXT: &[u8] = b"nocentraldb/content-key";

/// Why a change does not verify or does not open. Each variant names the
/// step that failed; none carries a key, a ciphertext or a plaintext.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// The change is not sealed (kind 0), so there is nothing to decrypt.
    NotSealed(u64),
    /// The change uses a suite this core does not know.
    UnknownSuite(u64),
    /// The header's author is not the member id of the given public keys.
    AuthorMismatch,
    /// The author's Ed25519 public key is not a valid key.
    AuthorKey,
    /// The signature does not verify, strictly, over the header bytes.
    Signature,
    /// The payload does not decrypt under the content key the header names.
    Decryption,
    /// The decrypted bytes are not a payload map.
    Payload(ChangeError),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::NotSealed(kind) => write!(f, "the change is not sealed: kind {kind}"),
            OpenError::UnknownSuite(suite) => write!(f, "unknown suite {suite}"),
            OpenError::AuthorMismatch => {
                f.write_str("the header's author is not the member of these public keys")
            }
            OpenError::AuthorKey => f.write_str("the author's public key is not an Ed25519 key"),
            OpenError::Signature => f.write_str("the signature does not verify"),
            OpenError::Decryption => {
                f.write_str("the payload does not decrypt under the key the header names")
            }
            OpenError::Payload(error) => write!(f, "the decrypted payload is invalid: {error}"),
        }
    }
}

impl std::error::Error for OpenError {}

/// The header signed by the author: the change of these header bytes and
/// their Ed25519 signature.
pub fn sign(author: &MemberKey, header: Header) -> Change {
    let signature = author.signing().sign(&header.encode());
    Change::new(header, signature.to_bytes().to_vec())
}

/// Verifies the change's signature against the author's public keys,
/// strictly, without any space key. This is what the relay can check.
pub fn verify(change: &Change, author: &MemberPublicKey) -> Result<(), OpenError> {
    if change.header().author != author.id() {
        return Err(OpenError::AuthorMismatch);
    }
    let key = VerifyingKey::from_bytes(&author.signing).map_err(|_| OpenError::AuthorKey)?;
    let signature = Signature::from_slice(change.signature()).map_err(|_| OpenError::Signature)?;
    key.verify_strict(change.header_bytes(), &signature)
        .map_err(|_| OpenError::Signature)
}

/// The payload, padded as sealing pads it: the canonical encoding is a
/// multiple of [`PADDING_BLOCK`] bytes.
pub fn pad(payload: &Payload) -> Payload {
    let mut padded = payload.clone();
    padded.padding = None;
    let length = padded.encode().len();
    if length.is_multiple_of(PADDING_BLOCK) {
        return padded;
    }
    padded.padding = Some(Vec::new());
    // The encoding with empty padding; n zero bytes add n, and from 24 and
    // 256 bytes on one and two more for the byte string's length.
    let base = padded.encode().len();
    let mut target = length.next_multiple_of(PADDING_BLOCK);
    loop {
        for growth in [0, 1, 2, 4] {
            let Some(zeros) = target.checked_sub(base + growth) else {
                continue;
            };
            padded.padding = Some(vec![0; zeros]);
            if padded.encode().len() == target {
                return padded;
            }
        }
        target += PADDING_BLOCK;
    }
}

/// Seals and signs a payload: a sealed change (kind 0) of suite 1 in this
/// space, by this author, with this sequence number, under this generation
/// of the space key. The nonce comes from the platform's secure generator.
pub fn seal(
    author: &MemberKey,
    space_key: &SpaceKey,
    space: SpaceId,
    sequence: u64,
    key_generation: u64,
    payload: &Payload,
) -> Change {
    let nonce = keys::random::<NONCE_LEN>();
    seal_with_nonce(
        author,
        space_key,
        space,
        sequence,
        key_generation,
        payload,
        *nonce,
    )
}

/// [`seal`] with a given nonce, for test vectors. A nonce must never be
/// used twice under one content key; outside test vectors, use [`seal`].
pub fn seal_with_nonce(
    author: &MemberKey,
    space_key: &SpaceKey,
    space: SpaceId,
    sequence: u64,
    key_generation: u64,
    payload: &Payload,
    nonce: [u8; NONCE_LEN],
) -> Change {
    let mut header = Header::new(
        space,
        author.id(),
        sequence,
        key_generation,
        Kind::SEALED,
        Vec::new(),
        SUITE,
    );
    let plaintext = Zeroizing::new(pad(payload).encode());
    let ciphertext = cipher(space_key, &header)
        .encrypt(
            &nonce.into(),
            Aad {
                msg: &plaintext,
                aad: &header.associated_data(),
            },
        )
        .expect("a payload is far below XChaCha20-Poly1305's length limit");
    header.payload = [nonce.as_slice(), &ciphertext].concat();
    sign(author, header)
}

/// Verifies a sealed change and decrypts its payload with the space key of
/// the generation its header names.
pub fn open(
    change: &Change,
    author: &MemberPublicKey,
    space_key: &SpaceKey,
) -> Result<Payload, OpenError> {
    let header = change.header();
    if header.kind != Kind::SEALED {
        return Err(OpenError::NotSealed(header.kind.0));
    }
    if header.suite != SUITE {
        return Err(OpenError::UnknownSuite(header.suite));
    }
    verify(change, author)?;
    let Some((nonce, ciphertext)) = header.payload.split_at_checked(NONCE_LEN) else {
        return Err(OpenError::Decryption);
    };
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce length");
    let plaintext = Zeroizing::new(
        cipher(space_key, header)
            .decrypt(
                &nonce.into(),
                Aad {
                    msg: ciphertext,
                    aad: &header.associated_data(),
                },
            )
            .map_err(|_| OpenError::Decryption)?,
    );
    Payload::decode(&plaintext).map_err(OpenError::Payload)
}

/// The cipher under the content key of the header's space, key generation
/// and suite: HKDF-SHA256 of the space key, without salt, with the info
/// `"nocentraldb/content-key" || space id || key generation || suite id`,
/// both numbers as 8 bytes, big-endian.
fn cipher(space_key: &SpaceKey, header: &Header) -> XChaCha20Poly1305 {
    let mut info = Vec::with_capacity(CONTENT_KEY_CONTEXT.len() + 16 + 8 + 8);
    info.extend_from_slice(CONTENT_KEY_CONTEXT);
    info.extend_from_slice(&header.space.0);
    info.extend_from_slice(&header.key_generation.to_be_bytes());
    info.extend_from_slice(&header.suite.to_be_bytes());
    let mut content_key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(None, space_key.bytes())
        .expand(&info, content_key.as_mut())
        .expect("32 bytes are far below HKDF-SHA256's output limit");
    XChaCha20Poly1305::new(&(*content_key).into())
}
