//! How a connection proves which member key it holds (RFC 0007, "Opening a
//! connection"; RFC 0003, "The member key"): the member signs
//! `H("nocentraldb/relay-auth" || relay id || nonce)` with its member
//! signing key, and the relay verifies it, strictly.

use ed25519_dalek::{Signature, VerifyingKey};
use ncdb_core::ids::MemberId;
use ncdb_core::keys::MemberPublicKey;

use sha2::{Digest, Sha256};

use crate::error::ErrorCode;

/// The context string in front of the relay id and the nonce.
pub const CONTEXT: &[u8] = b"nocentraldb/relay-auth";

/// What the member signs: SHA-256 of the context, the relay id and the
/// relay's nonce.
pub fn digest(relay_id: &[u8], nonce: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CONTEXT);
    hash.update(relay_id);
    hash.update(nonce);
    hash.finalize().into()
}

/// The member id the connection acts as, when `signature` verifies with
/// `key` over the digest of this relay's id and nonce; `unauthenticated`
/// otherwise.
pub fn verify(
    relay_id: &[u8],
    nonce: &[u8; 32],
    key: &MemberPublicKey,
    signature: &[u8],
) -> Result<MemberId, ErrorCode> {
    let verifying =
        VerifyingKey::from_bytes(&key.signing).map_err(|_| ErrorCode::Unauthenticated)?;
    let signature = Signature::from_slice(signature).map_err(|_| ErrorCode::Unauthenticated)?;
    verifying
        .verify_strict(&digest(relay_id, nonce), &signature)
        .map_err(|_| ErrorCode::Unauthenticated)?;
    Ok(key.id())
}
