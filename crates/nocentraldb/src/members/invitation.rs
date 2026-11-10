//! A member's invitation (RFC 0009, "Invitations and members"; decision
//! S12): its public identity, which the app carries to an admin of a space.
//! Nothing in it is secret, and no key bytes reach the app (RFC 0003,
//! "How the SDK hides keys").

use ncdb_core::keys::MemberPublicKey;
use ncdb_core::membership::Certificate;

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
/// Its text form, to show it to a person, is not built yet; two
/// fingerprints compare with `==`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub(crate) [u8; 32]);

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
