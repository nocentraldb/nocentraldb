//! A Yjs client id that names its author (RFC 0013, "Client ids name their
//! author"): derived from the member id and a nonce, so every item names
//! the member that wrote it, as the change's signature proves.

use sha2::{Digest, Sha256};

use crate::ids::MemberId;

/// What the hash starts with, so a client id is never another hash's value.
const LABEL: &[u8] = b"nocentraldb/yjs-client-id";

/// The largest integer a JavaScript number holds exactly, where Yjs keeps
/// client ids.
const MASK: u64 = (1 << 53) - 1;

/// The client id of `member` with `nonce`: the low 53 bits, as an unsigned
/// integer, of SHA-256(label || member id || nonce).
pub fn client_id(member: &MemberId, nonce: &[u8; 8]) -> u64 {
    let hash = Sha256::new()
        .chain_update(LABEL)
        .chain_update(member.0)
        .chain_update(nonce)
        .finalize();
    let low: [u8; 8] = hash[24..].try_into().expect("a SHA-256 hash has 32 bytes");
    u64::from_be_bytes(low) & MASK
}
