//! How a connection proves which member key it holds (RFC 0007, "Opening a
//! connection"; RFC 0003, decision 13): the member signs
//! `H("nocentraldb/relay-auth" || relay id || nonce)` with its member
//! signing key, and the relay verifies it, strictly.

use ncdb_core::ids::MemberId;
use ncdb_core::keys::MemberPublicKey;

use crate::error::ErrorCode;

/// The context string in front of the relay id and the nonce.
pub const CONTEXT: &[u8] = b"nocentraldb/relay-auth";

/// What the member signs: SHA-256 of the context, the relay id and the
/// relay's nonce.
pub fn digest(_relay_id: &[u8], _nonce: &[u8; 32]) -> [u8; 32] {
    [0; 32]
}

/// The member id the connection acts as, when `signature` verifies with
/// `key` over the digest of this relay's id and nonce; `unauthenticated`
/// otherwise.
pub fn verify(
    _relay_id: &[u8],
    _nonce: &[u8; 32],
    _key: &MemberPublicKey,
    _signature: &[u8],
) -> Result<MemberId, ErrorCode> {
    Err(ErrorCode::Unauthenticated)
}
