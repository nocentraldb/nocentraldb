//! The member's keys and where they live (RFC 0003, "The member key" and
//! "The account key"; RFC 0009, "The Rust SDK", decisions S19, S20 and
//! S26): the member key's two secrets, the database's account state and,
//! until the person stored it, the recovery code (RFC 0010, "The recovery
//! code"), kept in memory, in a key file the app names (`file`), or in the
//! platform keystore (`store`).
//!
//! They are kept as one key record, the same in a key file and in a
//! keystore entry, in format 2 (Carsten, 2026-11-12, NOC-285): the 16 ASCII
//! bytes `nocentraldb-keys`, the format byte `2`, the member signing secret
//! and the member key-agreement secret, then an account byte (`0` no
//! account yet, `1` the account key with its statement, `2` an agent's
//! certificate and statement) and a code byte (`0`, or `1` and the code's
//! 16 bytes). Format 1, 113 bytes, the member's secrets and the account
//! secret, is still read: it gets a recovery code and statement number 1
//! for its account key, and is written again as format 2.

#[cfg(target_arch = "wasm32")]
pub(crate) mod browser;
mod file;
pub(crate) mod store;

use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use ncdb_core::ids::MemberId;
use ncdb_core::keys::MemberKey;
use ncdb_core::membership::{AccountKeyStatement, Certificate, MemberKind};
use ncdb_core::recovery::RecoveryCode;
use zeroize::Zeroizing;

use crate::error::{Error, ErrorCode};

pub(crate) use file::from_file;
pub(crate) use store::{SecretStore, from_store};

/// What a key record starts with.
const MAGIC: &[u8; 16] = b"nocentraldb-keys";
/// The format this SDK writes.
const FORMAT: u8 = 2;
/// The format from before accounts, which this SDK still reads.
const FORMAT_1: u8 = 1;
/// The length of a format 1 record.
const LEN_1: usize = MAGIC.len() + 1 + 3 * 32;
/// The length of the member part of a format 2 record: the magic, the
/// format byte and the member's two secrets.
const MEMBER_LEN: usize = MAGIC.len() + 1 + 2 * 32;
/// The length of a statement in the record: the recovery key, the number
/// and the signature.
const STATEMENT_LEN: usize = 32 + 8 + 64;

/// The secret halves of the member key. The core's key cannot be copied,
/// and the checks of each space hold one, so the SDK keeps the secrets and
/// makes a key from them when it needs one.
#[derive(Clone)]
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
    /// challenge (RFC 0007, "Opening a connection"); natively only, until
    /// the browser syncs (NOC-175).
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(crate) fn signing(&self) -> &[u8; 32] {
        &self.signing
    }
}

/// What a database holds of its account (RFC 0010).
#[derive(Clone)]
pub(crate) enum Account {
    /// No account yet: a database opened for pairing that has not
    /// completed it.
    None,
    /// The account key, which certifies the member, and its statement.
    Holds {
        key: SigningKey,
        statement: AccountKeyStatement,
    },
    /// An agent's certificate of the account and its statement; an agent
    /// never holds the account key.
    Agent {
        certificate_signature: [u8; 64],
        statement: AccountKeyStatement,
    },
}

/// The keys of one database: the member key, its account and, until the
/// person stored it, the recovery code.
pub(crate) struct Keys {
    pub(crate) member: Secrets,
    pub(crate) account: Account,
    pub(crate) code: Option<RecoveryCode>,
    /// Whether the record read was format 1, so that it is written again
    /// as format 2.
    pub(crate) upgraded: bool,
}

/// A new account for `key`: a recovery code, and statement number 1 that
/// its recovery key signs.
fn new_account(key: SigningKey) -> (Account, RecoveryCode) {
    let code = RecoveryCode::generate();
    let statement =
        AccountKeyStatement::first(&code.recovery_key(), key.verifying_key().to_bytes());
    (Account::Holds { key, statement }, code)
}

impl Keys {
    /// The keys of a new database and a new account, from the platform's
    /// secure generator.
    pub(crate) fn generate() -> Keys {
        let (account, code) = new_account(SigningKey::from_bytes(&random()));
        Keys {
            member: Secrets {
                signing: random(),
                agreement: random(),
            },
            account,
            code: Some(code),
            upgraded: false,
        }
    }

    /// The keys of `member` with the account key `key`, as tests give
    /// them: the recovery code derives from the account key, so that
    /// databases given one account key are one account. It is confirmed.
    #[cfg(test)]
    pub(crate) fn of_account(member: Secrets, key: SigningKey) -> Keys {
        let bits: [u8; 16] = key.to_bytes()[..16].try_into().expect("16 bytes");
        let code = RecoveryCode::from_bits(bits);
        let statement =
            AccountKeyStatement::first(&code.recovery_key(), key.verifying_key().to_bytes());
        Keys {
            member,
            account: Account::Holds { key, statement },
            code: None,
            upgraded: false,
        }
    }

    /// The keys of a new database that recovers the account of `code` (RFC
    /// 0010, "The recovery code"): a member key and a new account key, with
    /// the statement the recovery key signs, numbered the larger of one
    /// more than the highest number the device learned, none on a new
    /// device, and the current time in Unix seconds (Carsten, 2026-11-12,
    /// NOC-285). The recovery key is dropped with `code`.
    pub(crate) fn recovered(code: &RecoveryCode, now_secs: u64) -> Keys {
        let key = SigningKey::from_bytes(&random());
        let number = now_secs.max(1);
        let statement =
            AccountKeyStatement::sign(&code.recovery_key(), key.verifying_key().to_bytes(), number);
        Keys {
            member: Secrets {
                signing: random(),
                agreement: random(),
            },
            account: Account::Holds { key, statement },
            code: None,
            upgraded: false,
        }
    }

    /// The keys of a new database opened for pairing: a member key and no
    /// account yet.
    pub(crate) fn pairing() -> Keys {
        Keys {
            member: Secrets {
                signing: random(),
                agreement: random(),
            },
            account: Account::None,
            code: None,
            upgraded: false,
        }
    }

    /// The key record, in format 2.
    pub(crate) fn encode(&self) -> Zeroizing<Vec<u8>> {
        let mut record = Zeroizing::new(Vec::with_capacity(MEMBER_LEN + 2 + 200 + 16));
        record.extend_from_slice(MAGIC);
        record.push(FORMAT);
        record.extend_from_slice(self.member.signing.as_ref());
        record.extend_from_slice(self.member.agreement.as_ref());
        let statement = |record: &mut Vec<u8>, statement: &AccountKeyStatement| {
            record.extend_from_slice(&statement.recovery_key);
            record.extend_from_slice(&statement.number.to_be_bytes());
            record.extend_from_slice(&statement.signature);
        };
        match &self.account {
            Account::None => record.push(0),
            Account::Holds { key, statement: s } => {
                record.push(1);
                record.extend_from_slice(Zeroizing::new(key.to_bytes()).as_ref());
                statement(&mut record, s);
            }
            Account::Agent {
                certificate_signature,
                statement: s,
            } => {
                record.push(2);
                record.extend_from_slice(&s.account_key);
                record.extend_from_slice(certificate_signature);
                statement(&mut record, s);
            }
        }
        match &self.code {
            None => record.push(0),
            Some(code) => {
                record.push(1);
                record.extend_from_slice(code.bits().as_ref());
            }
        }
        record
    }

    /// The keys of a key record, or `None` when the bytes are no record of
    /// format 1 or 2, or its statement does not verify. A record of format
    /// 1 gets a recovery code and statement number 1 for its account key,
    /// and is marked to be written again.
    pub(crate) fn decode(record: &[u8]) -> Option<Keys> {
        let rest = record.strip_prefix(MAGIC)?;
        let (&format, rest) = rest.split_first()?;
        let secret = |bytes: &[u8]| {
            let mut secret = Zeroizing::new([0; 32]);
            secret.copy_from_slice(bytes);
            secret
        };
        if format == FORMAT_1 {
            if record.len() != LEN_1 {
                return None;
            }
            let (account, code) = new_account(SigningKey::from_bytes(&secret(&rest[64..96])));
            return Some(Keys {
                member: Secrets {
                    signing: secret(&rest[..32]),
                    agreement: secret(&rest[32..64]),
                },
                account,
                code: Some(code),
                upgraded: true,
            });
        }
        if format != FORMAT || rest.len() < 2 * 32 + 2 {
            return None;
        }
        let member = Secrets {
            signing: secret(&rest[..32]),
            agreement: secret(&rest[32..64]),
        };
        let rest = &rest[64..];
        let statement = |account_key: [u8; 32], bytes: &[u8]| {
            let statement = AccountKeyStatement {
                recovery_key: bytes[..32].try_into().expect("32 bytes"),
                account_key,
                number: u64::from_be_bytes(bytes[32..40].try_into().expect("8 bytes")),
                signature: bytes[40..104].try_into().expect("64 bytes"),
            };
            statement.verify().then_some(statement)
        };
        let (account, rest) = match rest.split_first()? {
            (0, rest) => (Account::None, rest),
            (1, rest) if rest.len() > 32 + STATEMENT_LEN => {
                let key = SigningKey::from_bytes(&secret(&rest[..32]));
                let statement = statement(key.verifying_key().to_bytes(), &rest[32..])?;
                (
                    Account::Holds { key, statement },
                    &rest[32 + STATEMENT_LEN..],
                )
            }
            (2, rest) if rest.len() > 96 + STATEMENT_LEN => {
                let account_key = rest[..32].try_into().expect("32 bytes");
                let statement = statement(account_key, &rest[96..])?;
                let account = Account::Agent {
                    certificate_signature: rest[32..96].try_into().expect("64 bytes"),
                    statement,
                };
                (account, &rest[96 + STATEMENT_LEN..])
            }
            _ => return None,
        };
        let code = match rest {
            [0] => None,
            [1, bits @ ..] if bits.len() == 16 => {
                Some(RecoveryCode::from_bits(bits.try_into().expect("16 bytes")))
            }
            _ => return None,
        };
        Some(Keys {
            member,
            account,
            code,
            upgraded: false,
        })
    }
}

impl Account {
    /// The member's certificate, with its statement as key 6: signed by the
    /// account key a database holds, or an agent's as it was given. A
    /// database with no account yet has none.
    pub(crate) fn certificate(&self, member: MemberId, kind: MemberKind) -> Option<Certificate> {
        match self {
            Account::None => None,
            Account::Holds { key, statement } => {
                Some(Certificate::sign(key, member, kind).with_statement(statement.clone()))
            }
            Account::Agent {
                certificate_signature,
                statement,
            } => Some(Certificate {
                account_key: statement.account_key,
                member,
                kind: MemberKind::Agent,
                parent: None,
                signature: *certificate_signature,
                statement: Some(Box::new(statement.clone())),
            }),
        }
    }
}

/// Where a database's key record lives, so that it can be kept again when
/// it changes: on the person's `confirm()`, and when a pairing completes.
#[derive(Clone)]
pub(crate) enum Home {
    /// Nowhere: the database lives in memory.
    Memory,
    /// The key file the app named.
    File(PathBuf),
    /// The keystore entry of this user name.
    Store {
        store: std::sync::Arc<dyn SecretStore>,
        user: String,
    },
    /// IndexedDB, under this database name.
    #[cfg(target_arch = "wasm32")]
    Browser(String),
}

impl Home {
    /// Keeps `keys` as the database's key record.
    pub(crate) async fn save(&self, keys: &Keys) -> Result<(), Error> {
        match self {
            Home::Memory => Ok(()),
            Home::File(path) => file::save(path, keys),
            Home::Store { store, user } => store.set(user, &keys.encode()),
            #[cfg(target_arch = "wasm32")]
            Home::Browser(name) => browser::save(name, keys).await,
        }
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
