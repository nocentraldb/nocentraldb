//! The member's keys and where they live (RFC 0003, "The member key" and
//! "The account key"; RFC 0009, "The Rust SDK", decisions S19, S20 and
//! S26): the member key's two secrets and the account key that certifies
//! the member, drawn once per database and kept in memory, in a key file
//! the app names (`file`), or in the platform keystore (`store`).
//!
//! The three secrets are kept as one record of 113 bytes, the same in a
//! key file and in a keystore entry: the 16 ASCII bytes
//! `nocentraldb-keys`, the format byte `1`, then the member signing
//! secret, the member key-agreement secret and the account secret, 32
//! bytes each. No other format is read.

mod file;
pub(crate) mod store;

use ed25519_dalek::SigningKey;
use ncdb_core::keys::MemberKey;
use zeroize::Zeroizing;

use crate::error::{Error, ErrorCode};

pub(crate) use file::from_file;
pub(crate) use store::{SecretStore, from_store};

/// What a key record starts with.
const MAGIC: &[u8; 16] = b"nocentraldb-keys";
/// The format of the record this SDK writes and reads.
const FORMAT: u8 = 1;
/// The length of a key record.
const LEN: usize = MAGIC.len() + 1 + 3 * 32;

/// The secret halves of the member key. The core's key cannot be copied,
/// and the checks of each space hold one, so the SDK keeps the secrets and
/// makes a key from them when it needs one.
pub(crate) struct Secrets {
    signing: Zeroizing<[u8; 32]>,
    agreement: Zeroizing<[u8; 32]>,
}

impl Secrets {
    /// The member key's secrets as they are given.
    #[cfg(test)]
    pub(crate) fn new(signing: [u8; 32], agreement: [u8; 32]) -> Secrets {
        Secrets {
            signing: Zeroizing::new(signing),
            agreement: Zeroizing::new(agreement),
        }
    }

    /// The member key.
    pub(crate) fn key(&self) -> MemberKey {
        MemberKey::from_secrets(*self.signing, *self.agreement)
    }

    /// The secret half of the member signing key, which signs the relay's
    /// challenge (RFC 0007, "Opening a connection").
    pub(crate) fn signing(&self) -> &[u8; 32] {
        &self.signing
    }
}

/// The keys of one database: the member key and the account key that
/// certifies it, until the accounts RFC (RFC 0003, decision 17).
pub(crate) struct Keys {
    pub(crate) member: Secrets,
    pub(crate) account: SigningKey,
}

impl Keys {
    /// New keys, from the platform's secure generator.
    pub(crate) fn generate() -> Keys {
        Keys {
            member: Secrets {
                signing: random(),
                agreement: random(),
            },
            account: SigningKey::from_bytes(&random()),
        }
    }

    /// The key record.
    pub(crate) fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut record = Zeroizing::new(Vec::with_capacity(LEN));
        record.extend_from_slice(MAGIC);
        record.push(FORMAT);
        record.extend_from_slice(self.member.signing.as_ref());
        record.extend_from_slice(self.member.agreement.as_ref());
        record.extend_from_slice(Zeroizing::new(self.account.to_bytes()).as_ref());
        record
    }

    /// The keys of a key record, or `None` when the bytes are no record of
    /// this format.
    pub(crate) fn decode(record: &[u8]) -> Option<Keys> {
        let rest = record.strip_prefix(MAGIC)?.strip_prefix(&[FORMAT])?;
        if rest.len() != 3 * 32 {
            return None;
        }
        let secret = |at: usize| {
            let mut bytes = Zeroizing::new([0; 32]);
            bytes.copy_from_slice(&rest[at..at + 32]);
            bytes
        };
        Some(Keys {
            member: Secrets {
                signing: secret(0),
                agreement: secret(32),
            },
            account: SigningKey::from_bytes(&secret(64)),
        })
    }
}

/// 32 bytes from the platform's secure generator.
///
/// # Panics
///
/// When the platform has no secure generator; a key from a weaker source
/// is no key.
pub(crate) fn random() -> Zeroizing<[u8; 32]> {
    let mut bytes = Zeroizing::new([0; 32]);
    getrandom::fill(bytes.as_mut()).expect("the platform's secure random generator failed");
    bytes
}

/// A failure of the file system, without the path or any byte read:
/// `StorageFull` when the disk or the quota is full, `StorageFailed`
/// otherwise.
pub(crate) fn io(what: &str, error: &std::io::Error) -> Error {
    let code = match error.kind() {
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => {
            ErrorCode::StorageFull
        }
        _ => ErrorCode::StorageFailed,
    };
    Error::new(code, format!("{what}: {error}"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
