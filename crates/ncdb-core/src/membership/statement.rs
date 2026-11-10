//! The account key statement of RFC 0003 ("Account key statement"): the
//! account's recovery key names its current account key and a number.
//!
//! The recovery key signs `H("nocentraldb/account-key-statement" ||
//! canonical map of keys 1 to 3)`. A member certificate carries the
//! statement as its key 6 ([`super::Certificate`]), so a member checks a
//! chain of two: the statement against the recovery key, then the
//! certificate against the account key the statement names.

use std::fmt;

use ed25519_dalek::{Signature, Signer, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::cbor::{self, Value};
use crate::change::ChangeError;
use crate::keys::RecoveryKey;
use crate::redact;

use super::fields::{self, Fields};

/// An account key statement. Its keys are public, so its `Debug` output
/// shows them as hex.
#[derive(Clone, PartialEq, Eq)]
pub struct AccountKeyStatement {
    /// Key 1: the recovery key, the account's identity.
    pub recovery_key: [u8; 32],
    /// Key 2: the current account key.
    pub account_key: [u8; 32],
    /// Key 3: 1 for the first account key, one more with each new one.
    pub number: u64,
    /// Key 4: the recovery key's Ed25519 signature.
    pub signature: [u8; 64],
}

impl AccountKeyStatement {
    /// The context in front of the signed map.
    pub const CONTEXT: &'static [u8] = b"nocentraldb/account-key-statement";

    /// The statement, signed by `recovery`, that `account_key` is the
    /// account's key with this number.
    ///
    /// # Panics
    ///
    /// When `number` is 0: the first account key is number 1.
    pub fn sign(recovery: &RecoveryKey, account_key: [u8; 32], number: u64) -> AccountKeyStatement {
        assert_ne!(number, 0, "the first account key is number 1");
        let mut statement = AccountKeyStatement {
            recovery_key: recovery.public(),
            account_key,
            number,
            signature: [0; 64],
        };
        statement.signature = recovery.signing().sign(&statement.digest()).to_bytes();
        statement
    }

    /// What the recovery key signs: the SHA-256 hash of the context and the
    /// canonical map of keys 1 to 3.
    pub fn digest(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(AccountKeyStatement::CONTEXT);
        hash.update(cbor::encode(&fields::map(self.unsigned_fields())));
        hash.finalize().into()
    }

    /// Whether the signature verifies, strictly, with the recovery key of
    /// key 1. Whether that recovery key is the account's expected one is
    /// the caller's to check.
    pub fn verify(&self) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(&self.recovery_key) else {
            return false;
        };
        key.verify_strict(&self.digest(), &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    /// The canonical bytes of the statement map.
    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// Decodes canonical bytes of a statement map.
    pub fn decode(bytes: &[u8]) -> Result<AccountKeyStatement, ChangeError> {
        AccountKeyStatement::from_value(cbor::decode(bytes)?)
    }

    fn unsigned_fields(&self) -> Vec<(u64, Value)> {
        vec![
            (1, fields::bytes(&self.recovery_key)),
            (2, fields::bytes(&self.account_key)),
            (3, Value::Unsigned(self.number)),
        ]
    }

    pub(crate) fn to_value(&self) -> Value {
        let mut entries = self.unsigned_fields();
        entries.push((4, fields::bytes(&self.signature)));
        fields::map(entries)
    }

    /// A statement from its map. All four keys are required, the number is
    /// at least 1, and no other key may appear: RFC 0003 defines no
    /// extensions for it.
    pub(crate) fn from_value(value: Value) -> Result<AccountKeyStatement, ChangeError> {
        let mut map = Fields::of(value)?;
        let recovery_key = fields::fixed(map.required(1)?)?;
        let account_key = fields::fixed(map.required(2)?)?;
        let number = match fields::unsigned(map.required(3)?)? {
            0 => return Err(ChangeError::InvalidValue(3)),
            number => number,
        };
        let signature = fields::fixed(map.required(4)?)?;
        if let Some(key) = map.0.keys().next() {
            return Err(ChangeError::InvalidValue(*key));
        }
        Ok(AccountKeyStatement {
            recovery_key,
            account_key,
            number,
            signature,
        })
    }
}

impl fmt::Debug for AccountKeyStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Hex<'a>(&'a [u8]);
        impl fmt::Debug for Hex<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                redact::hex(f, self.0)
            }
        }
        f.debug_struct("AccountKeyStatement")
            .field("recovery_key", &Hex(&self.recovery_key))
            .field("account_key", &Hex(&self.account_key))
            .field("number", &self.number)
            .finish_non_exhaustive()
    }
}
