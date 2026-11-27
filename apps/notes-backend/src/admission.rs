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

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

/// The prefix of an admission's text.
pub const PREFIX: &str = "ncdb-admit-1:";

/// What the admission key signs under (RFC 0003's rule for signed input).
const CONTEXT: &[u8] = b"nocentraldb/relay-admission";

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
pub fn sign(key: &SigningKey, grant: &Grant) -> String {
    let unsigned = map(grant, None);
    let digest: [u8; 32] = Sha256::new()
        .chain_update(CONTEXT)
        .chain_update(&unsigned)
        .finalize()
        .into();
    let signature = key.sign(&digest).to_bytes();
    format!(
        "{PREFIX}{}",
        URL_SAFE_NO_PAD.encode(map(grant, Some(&signature)))
    )
}

/// The canonical map of `grant`, with the signature as key 5 when given.
fn map(grant: &Grant, signature: Option<&[u8; 64]>) -> Vec<u8> {
    let mut out = vec![0xa0 + 4 + u8::from(signature.is_some())];
    out.push(1);
    bytes(&mut out, &grant.relay_id);
    out.push(2);
    bytes(&mut out, &grant.identity);
    out.push(3);
    unsigned(&mut out, grant.until);
    out.push(4);
    unsigned(&mut out, grant.spaces);
    if let Some(signature) = signature {
        out.push(5);
        bytes(&mut out, signature);
    }
    out
}

/// CBOR's head of a major type, `major` << 5, with the shortest argument.
fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    match n {
        0..24 => out.push(major | n as u8),
        24..0x100 => out.extend([major | 24, n as u8]),
        0x100..0x1_0000 => {
            out.push(major | 25);
            out.extend((n as u16).to_be_bytes());
        }
        0x1_0000..0x1_0000_0000 => {
            out.push(major | 26);
            out.extend((n as u32).to_be_bytes());
        }
        _ => {
            out.push(major | 27);
            out.extend(n.to_be_bytes());
        }
    }
}

/// An unsigned integer.
fn unsigned(out: &mut Vec<u8>, n: u64) {
    head(out, 0x00, n);
}

/// A byte string.
fn bytes(out: &mut Vec<u8>, value: &[u8]) {
    head(out, 0x40, value.len() as u64);
    out.extend_from_slice(value);
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
        if !pair
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}
