//! The ids an app sees: an object id and a space id, 16 bytes each, and a
//! change id, 32 bytes. Each shows as lowercase hex, an object id as 32
//! digits (RFC 0009, Amendment 3).

use std::fmt;

macro_rules! hex_id {
    ($(#[$doc:meta])* $name:ident, $core:ident, $len:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub(crate) ncdb_core::ids::$core);

        impl fmt::Display for $name {
            /// The id as lowercase hex digits, two per byte.
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                for byte in self.0 .0 {
                    write!(f, "{byte:02x}")?;
                }
                Ok(())
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self)
            }
        }

    };
}

hex_id!(
    /// The id of an object: unique in its space, not across spaces.
    ObjectId,
    ObjectId,
    32
);
hex_id!(
    /// The id of a space.
    SpaceId,
    SpaceId,
    32
);
hex_id!(
    /// The id of a change.
    ChangeId,
    ChangeId,
    64
);
