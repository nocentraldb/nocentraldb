//! The admission of RFC 0015 ("The admission"): an app's backend signs, with
//! its admission key, that an account's identity may create spaces on one
//! relay until a time, up to a number of spaces.
//!
//! The admission key signs `H("nocentraldb/relay-admission" || canonical
//! map of keys 1 to 4)`, RFC 0003's rule for every signed input. As text,
//! an admission is `ncdb-admit-1:` and the canonical map's bytes in unpadded
//! base64url.

use std::fmt;
use std::str::FromStr;

use data_encoding::BASE64URL_NOPAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::cbor::{self, Value};
use crate::change::ChangeError;
use crate::membership::fields::{self, Fields};

/// What the text of every admission starts with.
pub const PREFIX: &str = "ncdb-admit-1:";

/// An admission. Everything in it is public, but its `Debug` output leaves
/// out the signature, which says nothing to a reader.
#[derive(Clone, PartialEq, Eq)]
pub struct Admission {
    /// Key 1: the relay it admits to, so it is worth nothing on another.
    /// The RFCs fix no length for a relay id, so the core takes any bytes.
    pub relay_id: Vec<u8>,
    /// Key 2: the identity it admits, the recovery key, or the account key
    /// of an account without one (RFC 0015, "The account's identity").
    pub identity: [u8; 32],
    /// Key 3: when it lapses, in Unix seconds.
    pub until: u64,
    /// Key 4: how many spaces the identity may create on this relay.
    pub spaces: u64,
    /// Key 5: the admission key's Ed25519 signature over keys 1 to 4.
    pub signature: [u8; 64],
}

/// Why a text is not an admission. Nothing is read from such a text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// It does not start with `ncdb-admit-1:`.
    Prefix,
    /// The rest is not unpadded base64url.
    Encoding,
    /// The bytes are not an admission's canonical map.
    Map,
}

impl Admission {
    /// The context in front of the signed map.
    pub const CONTEXT: &'static [u8] = b"nocentraldb/relay-admission";

    /// The admission, signed by `admission_key`, that `identity` may create
    /// `spaces` spaces on the relay `relay_id` until `until`.
    pub fn sign(
        admission_key: &SigningKey,
        relay_id: Vec<u8>,
        identity: [u8; 32],
        until: u64,
        spaces: u64,
    ) -> Admission {
        let mut admission = Admission {
            relay_id,
            identity,
            until,
            spaces,
            signature: [0; 64],
        };
        admission.signature = admission_key.sign(&admission.digest()).to_bytes();
        admission
    }

    /// What the admission key signs: the SHA-256 hash of the context and
    /// the canonical map of keys 1 to 4.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(Admission::CONTEXT);
        hash.update(cbor::encode(&fields::map(self.unsigned_fields())));
        hash.finalize().into()
    }

    /// Whether the signature verifies, strictly, with `admission_key`.
    /// Whether the relay configures that key, names its own relay id and
    /// has not passed `until` is the relay's to check.
    pub fn verify(&self, admission_key: &[u8; 32]) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(admission_key) else {
            return false;
        };
        key.verify_strict(&self.digest(), &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    /// The canonical bytes of the admission map.
    pub fn encode(&self) -> Vec<u8> {
        let mut entries = self.unsigned_fields();
        entries.push((5, fields::bytes(&self.signature)));
        cbor::encode(&fields::map(entries))
    }

    /// Decodes canonical bytes of an admission map. All five keys are
    /// required, each of its type and length, and no other key may appear.
    pub fn decode(bytes: &[u8]) -> Result<Admission, ChangeError> {
        let mut map = Fields::of(cbor::decode(bytes)?)?;
        let admission = Admission {
            relay_id: byte_string(map.required(1)?)?,
            identity: fields::fixed(map.required(2)?)?,
            until: fields::unsigned(map.required(3)?)?,
            spaces: fields::unsigned(map.required(4)?)?,
            signature: fields::fixed(map.required(5)?)?,
        };
        if let Some(key) = map.0.keys().next() {
            return Err(ChangeError::InvalidValue(*key));
        }
        Ok(admission)
    }

    fn unsigned_fields(&self) -> Vec<(u64, Value)> {
        vec![
            (1, fields::bytes(&self.relay_id)),
            (2, fields::bytes(&self.identity)),
            (3, Value::Unsigned(self.until)),
            (4, Value::Unsigned(self.spaces)),
        ]
    }
}

/// Key 1's bytes, of any length.
fn byte_string((key, value): (u64, Value)) -> Result<Vec<u8>, ChangeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

/// `ncdb-admit-1:` and unpadded base64url of the canonical map.
impl fmt::Display for Admission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}", BASE64URL_NOPAD.encode(&self.encode()))
    }
}

/// Reads an admission's text: the prefix, strict unpadded base64url, and
/// the canonical map ([`Admission::decode`]).
impl FromStr for Admission {
    type Err = ReadError;

    fn from_str(text: &str) -> Result<Admission, ReadError> {
        let body = text.strip_prefix(PREFIX).ok_or(ReadError::Prefix)?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| ReadError::Encoding)?;
        Admission::decode(&bytes).map_err(|_| ReadError::Map)
    }
}

impl fmt::Debug for Admission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Hex<'a>(&'a [u8]);
        impl fmt::Debug for Hex<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                crate::redact::hex(f, self.0)
            }
        }
        f.debug_struct("Admission")
            .field("relay_id", &Hex(&self.relay_id))
            .field("identity", &Hex(&self.identity))
            .field("until", &self.until)
            .field("spaces", &self.spaces)
            .finish_non_exhaustive()
    }
}
