//! Ids without a coordinator (RFC 0002, "Ids without a coordinator").
//!
//! Object and space ids are 128 random bits from the platform's secure
//! generator. A change id is the SHA-256 hash of the frame version and the
//! signed header bytes; an operation id is a change id plus the operation's
//! position. Member ids are RFC 0003's: the SHA-256 hash of a member's public
//! keys, here only carried as 32 bytes.

use std::fmt;

/// The id of a space: 16 random bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpaceId(pub [u8; 16]);

/// The id of an object: 16 random bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub [u8; 16]);

/// The id of a member: RFC 0003's SHA-256 hash of its public keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberId(pub [u8; 32]);

/// The id of a change: SHA-256 of `"nocentraldb/change-id"`, the frame
/// version as one byte, and the signed header bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChangeId(pub [u8; 32]);

/// The id of an operation: its change's id and its position in the change's
/// operation array, counted from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationId {
    /// The change that holds the operation.
    pub change: ChangeId,
    /// The operation's position in the change's operation array.
    pub position: u64,
}

impl SpaceId {
    /// A new space id from the platform's secure random generator.
    pub fn random() -> SpaceId {
        todo!()
    }
}

impl ObjectId {
    /// A new object id from the platform's secure random generator.
    pub fn random() -> ObjectId {
        todo!()
    }
}

impl ChangeId {
    /// The context string in front of every hashed change.
    pub const CONTEXT: &'static [u8] = b"nocentraldb/change-id";

    /// The id of the change with this frame version and signed header bytes.
    pub fn compute(_frame_version: u8, _header_bytes: &[u8]) -> ChangeId {
        todo!()
    }
}

impl fmt::Display for ChangeId {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}
