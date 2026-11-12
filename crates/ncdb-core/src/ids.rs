//! Ids without a coordinator (RFC 0002, "Ids without a coordinator").
//!
//! Object and space ids are 128 random bits from the platform's secure
//! generator. A change id is the SHA-256 hash of the frame version and the
//! signed header bytes; an operation id is a change id plus the operation's
//! position. Member ids are RFC 0003's: the SHA-256 hash of a member's public
//! keys, here only carried as 32 bytes.
//!
//! The ids a plugin sees, [`MemberId`], [`ChangeId`] and [`OperationId`],
//! live in `ncdb-plugin` (RFC 0011, "What a plugin depends on"), and the
//! core re-exports them as its own.

use std::fmt;

use sha2::{Digest, Sha256};

pub use ncdb_plugin::{ChangeId, MemberId, OperationId};

use crate::redact;

/// The id of a space: 16 random bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpaceId(pub [u8; 16]);

/// The id of an object: 16 random bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub [u8; 16]);

/// Ids show as hex: `SpaceId(0001…)`.
macro_rules! debug_as_hex {
    ($($id:ident),*) => {$(
        impl fmt::Debug for $id {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!(stringify!($id), "("))?;
                redact::hex(f, &self.0)?;
                f.write_str(")")
            }
        }
    )*};
}

debug_as_hex!(SpaceId, ObjectId);

impl SpaceId {
    /// A new space id from the platform's secure random generator.
    pub fn random() -> SpaceId {
        SpaceId(random())
    }
}

impl ObjectId {
    /// A new object id from the platform's secure random generator.
    pub fn random() -> ObjectId {
        ObjectId(random())
    }
}

/// Random bytes from the platform's secure generator: `getrandom`
/// natively, `crypto.getRandomValues` in the browser.
///
/// # Panics
///
/// When the platform has no secure generator. An id from a weaker source
/// could collide, so the core does not fall back to one.
fn random<const N: usize>() -> [u8; N] {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).expect("the platform's secure random generator failed");
    bytes
}

/// The context string in front of every hashed change.
pub const CHANGE_ID_CONTEXT: &[u8] = b"nocentraldb/change-id";

/// The id of the change with this frame version and signed header bytes.
pub fn change_id(frame_version: u8, header_bytes: &[u8]) -> ChangeId {
    let mut hash = Sha256::new();
    hash.update(CHANGE_ID_CONTEXT);
    hash.update([frame_version]);
    hash.update(header_bytes);
    ChangeId(hash.finalize().into())
}
