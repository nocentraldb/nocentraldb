//! The members a space lists (RFC 0009, "Invitations and members"; the
//! Rust shapes the owner decided on NOC-203, Carsten, 2026-11-11): each
//! current member's id, its account's fingerprint, its kind from its
//! certificate, its role and whether it is this member. The id is what
//! [`Members::remove`] and [`Members::set_role`] take.

use ncdb_core::membership;

use super::{Fingerprint, Members, Role};
use crate::db::lock;
use crate::error::{Error, ErrorCode};

pub use crate::id::MemberId;

/// What a member is, from its certificate (RFC 0003, "The account key").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MemberKind {
    /// A person's device.
    Device,
    /// An agent.
    Agent,
    /// A sub-agent, certified by its parent agent.
    SubAgent,
}

impl MemberKind {
    fn of(kind: membership::MemberKind) -> MemberKind {
        match kind {
            membership::MemberKind::Device => MemberKind::Device,
            membership::MemberKind::Agent => MemberKind::Agent,
            membership::MemberKind::SubAgent => MemberKind::SubAgent,
        }
    }
}

/// One current member of a space, from [`Members::list`]. It is
/// `#[non_exhaustive]`: an app reads its fields, and a later field is no
/// breaking change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MemberInfo {
    /// Its id, which [`Members::remove`] and [`Members::set_role`] take.
    pub id: MemberId,
    /// The fingerprint of the account its certificate names. A membership
    /// written before that account was linked into another keeps it
    /// (RFC 0010, "Linking accounts drawn per database").
    pub account: Fingerprint,
    /// The fingerprint of the account that `account` is linked into, when
    /// this database knows the link: from its own account's link
    /// statements. `None` when `account` is linked into no other, and for
    /// another person's linked accounts, whose links this database does
    /// not learn (the owner's NOC-307 A1 and B3).
    pub linked_to: Option<Fingerprint>,
    /// Whether it is a device, an agent or a sub-agent.
    pub kind: MemberKind,
    /// Its role in the space.
    pub role: Option<Role>,
    /// Whether it is this database's own member (`self` is a Rust
    /// keyword).
    pub is_self: bool,
}

impl Members<'_> {
    /// The current members of the space, in member-id order, as this copy's
    /// membership state holds them: a removed member is not listed, and a
    /// member that has not seen a change yet shows the space as it was
    /// before it. Every member is listed, an agent included.
    pub async fn list(&self) -> Result<Vec<MemberInfo>, Error> {
        let inner = lock(self.space.inner());
        let space = self.space.id();
        let state = inner.spaces.get(&space.0).ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("the copy holds no space {space}"),
            )
        })?;
        let me = inner.member.key().id();
        let linked = crate::account::linked(&inner)?;
        let resolved = state.checks.resolved();
        Ok(resolved
            .members()
            .filter_map(|(id, role)| {
                let member = resolved.member(id)?;
                let account = Fingerprint::of(&member.certificate);
                let linked_to = linked
                    .get(&member.certificate.account_key)
                    .copied()
                    .filter(|into| *into != account);
                Some(MemberInfo {
                    id: MemberId(id),
                    account,
                    linked_to,
                    kind: MemberKind::of(member.certificate.kind),
                    role: Some(Role::of(role)),
                    is_self: id == me,
                })
            })
            .collect())
    }
}
