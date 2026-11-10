//! A member's invitation (RFC 0009, "Invitations and members"; decision
//! S12): its public identity, which the app carries to an admin of a space.
//! Nothing in it is secret, and no key bytes reach the app (RFC 0003,
//! "How the SDK hides keys").

use ncdb_core::keys::MemberPublicKey;
use ncdb_core::membership::Certificate;
use sha2::{Digest, Sha256};

use crate::db::Inner;

/// The public identity of a member, from [`Db::invitation`](crate::Db::invitation):
/// its member id, its two public keys, the certificate its account signed,
/// and its account's [`Fingerprint`]. An admin adds it to a space with
/// [`Members::add`](crate::Members::add).
///
/// Its text form, to carry it between devices, is not built yet (NOC-141).
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
