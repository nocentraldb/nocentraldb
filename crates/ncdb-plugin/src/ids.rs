//! The ids a plugin sees (RFC 0002, "Ids without a coordinator"): a
//! member's id, a change's id, and an operation's id, its change's id and
//! its position in the change. `ncdb-core` re-exports them as its own.

use std::fmt;

use crate::redact;

/// The id of a member: RFC 0003's SHA-256 hash of its public keys.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberId(pub [u8; 32]);

/// The id of a change: SHA-256 of `"nocentraldb/change-id"`, the frame
/// version as one byte, and the signed header bytes. The core computes it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

/// Ids show as hex: `ChangeId(0001…)`.
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

debug_as_hex!(MemberId, ChangeId);

impl fmt::Display for ChangeId {
    /// The id in lower-case hex.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        redact::hex(f, &self.0)
    }
}
