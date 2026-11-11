//! A member's invitation (RFC 0009, "Invitations and members"): its public
//! identity, which the app carries to an admin of a space. Nothing in it is
//! secret, and no key bytes reach the app (RFC 0003, "How the SDK hides keys").
//!
//! Its text form is `ncdb-invite-1:` and unpadded base64url of a fixed
//! binary layout that starts with a version byte (RFC 0003, "Invitation";
//! the owner's NOC-141 A, Carsten, 2026-11-10). The layout holds only what
//! a reader cannot rebuild: the member id, certificate key 4 and the
//! statement's account key are derived, so no two fields can disagree.

use std::str::FromStr;

use data_encoding::BASE64URL_NOPAD;
use ncdb_core::keys::MemberPublicKey;
use ncdb_core::membership::{AccountKeyStatement, Certificate, MemberKind};
use sha2::{Digest, Sha256};

use crate::db::Inner;
use crate::error::{Error, ErrorCode};

/// What every invitation's text starts with.
const PREFIX: &str = "ncdb-invite-1:";
/// The layout's version byte.
const VERSION: u8 = 1;
/// The length of the layout without a statement, and with one.
const PLAIN: usize = 163;
const WITH_STATEMENT: usize = 267;

/// The public identity of a member, from [`Db::invitation`](crate::Db::invitation):
/// its member id, its two public keys, the certificate its account signed,
/// and its account's [`Fingerprint`]. An admin adds it to a space with
/// [`Members::add`](crate::Members::add).
///
/// Its text form, to carry it between devices, is what `Display` writes
/// and `FromStr` reads: `text.parse::<Invitation>()` checks only the
/// layout, and [`Members::add`](crate::Members::add) checks the certificate.
#[derive(Clone, PartialEq, Eq)]
pub struct Invitation {
    pub(crate) key: MemberPublicKey,
    pub(crate) certificate: Certificate,
}

/// The fingerprint of an account, which a person compares once before an
/// admin adds a member of an account new to its database, the way
/// messaging apps compare safety numbers (RFC 0003, "Sharing a space key
/// with a new member").
///
/// It shows as 60 digits in twelve blocks of five, as RFC 0010 gives it
/// ("The fingerprint"), for a person to compare; two fingerprints also
/// compare with `==`. RFC 0010 computes it over the account's recovery
/// key, which is not built yet, so it is computed over the account key.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub(crate) [u8; 32]);

impl Fingerprint {
    /// The context string in front of the account key (RFC 0010, "The
    /// fingerprint").
    const CONTEXT: &'static [u8] = b"nocentraldb/account-fingerprint";

    /// The fingerprint's twelve blocks of five digits, the most significant
    /// first: `SHA-256(CONTEXT || account key)` read as a big-endian
    /// integer, mod 10^60. Each division by 10^5 gives the next block from
    /// the end.
    fn blocks(&self) -> [u32; 12] {
        let mut hash: [u8; 32] = Sha256::new()
            .chain_update(Self::CONTEXT)
            .chain_update(self.0)
            .finalize()
            .into();
        let mut blocks = [0; 12];
        for block in blocks.iter_mut().rev() {
            let mut remainder = 0u64;
            for byte in hash.iter_mut() {
                let current = (remainder << 8) | u64::from(*byte);
                *byte = (current / 100_000) as u8;
                remainder = current % 100_000;
            }
            *block = remainder as u32;
        }
        blocks
    }
}

impl std::fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, block) in self.blocks().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{block:05}")?;
        }
        Ok(())
    }
}

impl Invitation {
    /// The invitation of the database's own member.
    pub(crate) fn of(inner: &Inner) -> Invitation {
        let member = inner.member.key();
        Invitation {
            key: member.public(),
            certificate: Certificate::sign(&inner.account, member.id(), inner.kind),
        }
    }

    /// The fingerprint of the account that certified the member.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint(self.certificate.account_key)
    }
}

impl Invitation {
    /// The layout of RFC 0003, "Invitation".
    fn bytes(&self) -> Vec<u8> {
        let certificate = &self.certificate;
        let mut bytes = Vec::with_capacity(WITH_STATEMENT);
        bytes.push(VERSION);
        bytes.extend_from_slice(&self.key.signing);
        bytes.extend_from_slice(&self.key.agreement);
        bytes.extend_from_slice(&certificate.account_key);
        bytes.push(match certificate.kind {
            MemberKind::Device => 1,
            MemberKind::Agent => 2,
            MemberKind::SubAgent => unreachable!("a database's member is a device or an agent"),
        });
        bytes.extend_from_slice(&certificate.signature);
        match certificate.statement.as_deref() {
            None => bytes.push(0),
            Some(statement) => {
                bytes.push(1);
                bytes.extend_from_slice(&statement.recovery_key);
                bytes.extend_from_slice(&statement.number.to_be_bytes());
                bytes.extend_from_slice(&statement.signature);
            }
        }
        bytes
    }

    /// Reads the layout: its shape, not its signatures.
    fn read(bytes: &[u8]) -> Result<Invitation, &'static str> {
        let version = *bytes.first().ok_or("it is empty")?;
        if version != VERSION {
            return Err("its version is unknown");
        }
        if bytes.len() < PLAIN {
            return Err("it is too short");
        }
        let at = |offset: usize| -> [u8; 32] { bytes[offset..offset + 32].try_into().unwrap() };
        let kind = match bytes[97] {
            1 => MemberKind::Device,
            2 => MemberKind::Agent,
            _ => return Err("its kind is neither a device nor an agent"),
        };
        let account_key = at(65);
        let statement = match (bytes[162], bytes.len()) {
            (0, PLAIN) => None,
            (1, WITH_STATEMENT) => {
                let number = u64::from_be_bytes(bytes[195..203].try_into().unwrap());
                if number == 0 {
                    return Err("its account key number is 0");
                }
                Some(Box::new(AccountKeyStatement {
                    recovery_key: at(163),
                    account_key,
                    number,
                    signature: bytes[203..267].try_into().unwrap(),
                }))
            }
            (0 | 1, _) => return Err("its length does not match its statement byte"),
            _ => return Err("its statement byte is neither 0 nor 1"),
        };
        let key = MemberPublicKey {
            signing: at(1),
            agreement: at(33),
        };
        let certificate = Certificate {
            account_key,
            member: key.id(),
            kind,
            parent: None,
            signature: bytes[98..162].try_into().unwrap(),
            statement,
        };
        Ok(Invitation { key, certificate })
    }
}

/// `ncdb-invite-1:` and unpadded base64url of the layout.
impl std::fmt::Display for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{PREFIX}{}", BASE64URL_NOPAD.encode(&self.bytes()))
    }
}

/// Reads an invitation's text. A text that does not follow the layout is
/// refused with `InvalidValue`: another prefix, anything but unpadded
/// base64url, an unknown version, a kind other than a device or an agent,
/// a statement byte other than 0 or 1, a length that does not match it, or
/// account key number 0.
impl FromStr for Invitation {
    type Err = Error;

    fn from_str(text: &str) -> Result<Invitation, Error> {
        let invalid = |why: &str| {
            Error::new(
                ErrorCode::InvalidValue,
                format!("the text is not an invitation: {why}"),
            )
        };
        let body = text
            .strip_prefix(PREFIX)
            .ok_or_else(|| invalid("it does not start with ncdb-invite-1:"))?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| invalid("it is not unpadded base64url"))?;
        Invitation::read(&bytes).map_err(invalid)
    }
}

impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitation").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fingerprint").finish_non_exhaustive()
    }
}
