//! The ids an app sees: an object id and a space id, 16 bytes each, and a
//! change id, 32 bytes. Each shows and reads back as lowercase hex, an
//! object id as 32 digits (RFC 0009, Amendment 3).

use std::fmt;
use std::str::FromStr;

use crate::error::{Error, ErrorCode};

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

        impl FromStr for $name {
            type Err = Error;

            /// Reads an id from its lowercase hex digits; anything else is
            /// refused with `InvalidValue`.
            fn from_str(text: &str) -> Result<$name, Error> {
                let invalid = || {
                    Error::new(
                        ErrorCode::InvalidValue,
                        concat!("not ", stringify!($name), " in ", $len, " lowercase hex digits"),
                    )
                };
                if text.len() != $len {
                    return Err(invalid());
                }
                let mut bytes = [0; $len / 2];
                for (i, byte) in bytes.iter_mut().enumerate() {
                    let pair = &text[2 * i..2 * i + 2];
                    if !pair.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
                        return Err(invalid());
                    }
                    *byte = u8::from_str_radix(pair, 16).map_err(|_| invalid())?;
                }
                Ok($name(ncdb_core::ids::$core(bytes)))
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
