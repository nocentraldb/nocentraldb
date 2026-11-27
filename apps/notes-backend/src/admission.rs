//! An admission, as RFC 0015 defines it ("The admission"): a canonical
//! CBOR map of the relay id (key 1), the account's identity (key 2), the
//! time it lapses in Unix seconds (key 3) and the number of spaces the
//! identity may create (key 4), signed as key 5 by the app's admission key
//! over SHA-256 of `"nocentraldb/relay-admission"` and the map of keys 1 to
//! 4. As text it is `ncdb-admit-1:` and the map's bytes in unpadded
//! base64url.
//!
//! The format is all a backend needs: Ed25519, SHA-256, base64url and five
//! CBOR heads, in any language. `docs/rfc/vectors/0015/` holds vectors to
//! check an implementation against.

use ed25519_dalek::SigningKey;

/// The prefix of an admission's text.
pub const PREFIX: &str = "ncdb-admit-1:";

/// What one admission grants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    /// The relay it admits to, `NCDB_RELAY_ID`.
    pub relay_id: [u8; 16],
    /// The account's identity: 32 bytes, `db.account.identity` decoded.
    pub identity: [u8; 32],
    /// When it lapses, in Unix seconds.
    pub until: u64,
    /// How many spaces the identity may create on the relay.
    pub spaces: u64,
}

/// The admission's text for `grant`, signed with `key`.
pub fn sign(_key: &SigningKey, _grant: &Grant) -> String {
    String::new()
}

/// 32 bytes written as 64 hex digits, as `db.account.identity` gives an
/// identity; none for any other text.
pub fn identity(hex: &str) -> Option<[u8; 32]> {
    let digits = hex.as_bytes();
    if digits.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        let pair = std::str::from_utf8(&digits[2 * i..2 * i + 2]).ok()?;
        if !pair.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}
