//! The members of a space (RFC 0009, "Invitations and members"): an admin
//! adds a member by its invitation, which writes a membership entry and a
//! key envelope of the current generation's space key to the new member
//! (RFC 0008, "Membership entry" and "Key envelope"; RFC 0003, "Sharing a
//! space key with a new member").
//!
//! [`Members::list`] gives the current members, and [`Members::remove`]
//! and [`Members::set_role`] change them (the Rust shapes the owner decided
//! on NOC-203).
//!
//! A member leaves a space with [`Space::leave`](crate::Space::leave).
//!
//! **What this does not do yet.** Adding a list of invitations, and
//! re-wrapping person-key records in a rotation: a call that rotates
//! refuses a space with a person key.

use std::collections::BTreeSet;

use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::membership::{self, Action, Entry, MembershipEntry, Resolved};
use ncdb_core::view::View;

use crate::db::{Inner, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::space::{Space, not_stopped, writable};

mod batch;
mod change;
mod info;
mod invitation;
mod leave;
mod notice;
mod rotate;

use batch::Batch;

pub use info::{MemberId, MemberInfo, MemberKind};
pub use invitation::{Fingerprint, Invitation};
pub use notice::{Notice, NoticeCode};

/// The members of one space, from [`Space::members`].
#[derive(Debug)]
pub struct Members<'a> {
    pub(crate) space: &'a Space,
}

/// A member's role in a space (RFC 0009, decision S13; RFC 0008, "Roles
/// and rights").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Reads the whole space.
    Reader,
    /// Also writes content.
    Writer,
    /// Also adds members, changes their roles and removes them.
    Admin,
}

impl Role {
    /// The SDK's name of a role of the core's membership.
    pub(crate) fn of(role: membership::Role) -> Role {
        match role {
            membership::Role::Reader => Role::Reader,
            membership::Role::Writer => Role::Writer,
            membership::Role::Admin => Role::Admin,
        }
    }

    /// The role's name in an error's text, with its article.
    fn name(self) -> &'static str {
        match self {
            Role::Reader => "a member",
            Role::Writer => "a writer or an admin",
            Role::Admin => "an admin",
        }
    }
}

impl From<Role> for membership::Role {
    fn from(role: Role) -> membership::Role {
        match role {
            Role::Reader => membership::Role::Reader,
            Role::Writer => membership::Role::Writer,
            Role::Admin => membership::Role::Admin,
        }
    }
}

/// The options of [`Members::add`]: the new member's role, and the
/// fingerprint of its account when a person compared it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddMember {
    /// The role the new member gets.
    pub role: Role,
    /// The fingerprint of the invitation's account, which a person
    /// compared. It is needed when the account is new to this database.
    pub fingerprint: Option<Fingerprint>,
}

impl Members<'_> {
    /// Adds the member of `invitation` to the space with the role
    /// `options` gives: writes the membership entry and the new member's
    /// key envelope of the current generation, after the heads of the
    /// space's control DAG, and folds them, on the device. The new member
    /// reads the whole history of the space once it syncs, which the
    /// returned [`Notice`] says (`MemberAdded`).
    ///
    /// Only an admin adds members: another member is refused with
    /// `NotAllowed`, one that was removed, or that the relay answered
    /// `not_member` or `removed_member` for the space, with `NotMember`
    /// (RFC 0009, "Errors"). A member of the space is refused with
    /// `AlreadyMember`. A former member is added again after a rotation
    /// (RFC 0008, M5): the space key moves to a fresh generation, with an
    /// envelope for every member, and the member is added from it, so what
    /// it wrote while it was not a member never counts again. A space that
    /// holds a person key is refused with `InvalidValue` then, since
    /// re-wrapping its records in a rotation is not built.
    ///
    /// An invitation whose certificate does not hold is refused with
    /// `InvalidCertificate`: one its account did not sign, one that
    /// certifies another member than the invitation's keys, or a
    /// sub-agent's. An account other than this database's own is new to it,
    /// since each database draws its own account key (RFC 0009, S20),
    /// until a person confirmed its fingerprint: its member is refused
    /// with `UnknownAccount` unless `options` carry the invitation's
    /// fingerprint. Once an add with the fingerprint succeeds, the database
    /// remembers the account, in its file or in memory, and a later add of
    /// any member of it needs no fingerprint (NOC-154 point 3).
    pub async fn add(&self, invitation: &Invitation, options: AddMember) -> Result<Notice, Error> {
        let mut inner = lock(self.space.inner());
        let space = self.space.id();
        writable(&inner, space)?;
        not_stopped(&inner, space)?;
        let resolved = inner.spaces[&space.0].checks.resolved();
        let former = allowed(resolved, inner.member.key().id(), invitation.key.id())?;
        certified(invitation)?;
        let confirming = known(&inner, invitation, options.fingerprint)?;
        let member = inner.member.key();
        let Inner {
            view,
            spaces,
            member: secrets,
            ..
        } = &mut *inner;
        let state = spaces.get_mut(&space.0).expect("a writable space is held");
        let mut batch = Batch::new(view, state, &member, space.0);
        if former {
            rotate::rotatable(state)?;
            rotate::rotate(&mut batch, state, &member, space.0, BTreeSet::new(), None);
        }
        let added = batch.write(
            &member,
            Entry::Membership(MembershipEntry {
                subject: invitation.key.id(),
                action: Action::Add {
                    role: options.role.into(),
                    key: invitation.key,
                    certificate: Box::new(invitation.certificate.clone()),
                    from_generation: batch.generation,
                },
            }),
            None,
        );
        let sealed = envelope::seal(batch.key(state), space.0, batch.generation, &invitation.key)
            .expect("a member's public key takes an envelope");
        batch.write(
            &member,
            Entry::KeyEnvelope(sealed),
            Some(BTreeSet::from([added])),
        );
        batch.commit(view, state, secrets)?;
        if confirming {
            confirm(view, &invitation.certificate.account_key)?;
        }
        crate::sync::wake(&inner);
        Ok(Notice::MEMBER_ADDED)
    }
}

/// Whether `member` holds at least the role `needed` in the membership
/// state `resolved` (RFC 0008, "Roles and rights"): a member with a lower
/// role is refused with `NotAllowed`, one without a role, which was
/// removed, with `NotMember` (RFC 0009, "Errors"). `doing` names what the
/// role is needed for.
pub(crate) fn holds(
    resolved: &Resolved,
    member: ncdb_core::ids::MemberId,
    needed: membership::Role,
    doing: &str,
) -> Result<(), Error> {
    match resolved.role(member) {
        Some(role) if role >= needed => Ok(()),
        Some(_) => Err(Error::new(
            ErrorCode::NotAllowed,
            format!("only {} of the space {doing}", Role::of(needed).name()),
        )),
        None => Err(Error::new(
            ErrorCode::NotMember,
            "the member is no longer a member of the space",
        )),
    }
}

/// Whether `member` may add `subject` in the membership state `resolved`:
/// it is an admin, and `subject` is not a member. Says whether `subject`
/// is a former member, whose add is a re-grant that rotates first (RFC
/// 0008, M5).
fn allowed(
    resolved: &Resolved,
    member: ncdb_core::ids::MemberId,
    subject: ncdb_core::ids::MemberId,
) -> Result<bool, Error> {
    holds(resolved, member, membership::Role::Admin, "adds members")?;
    match resolved.member(subject) {
        None => Ok(false),
        Some(state) if state.role.is_some() => Err(Error::new(
            ErrorCode::AlreadyMember,
            "the invitation's member is a member of the space already",
        )),
        Some(_) => Ok(true),
    }
}

/// Whether the certificate of `invitation` holds, as RFC 0008's fold
/// checks an add ("Membership entry"): it certifies the member of the
/// invitation's keys as a device or an agent, and its account signed it.
/// A sub-agent's certificate is signed by its parent when it is added
/// (RFC 0008, M25), which is not built.
fn certified(invitation: &Invitation) -> Result<(), Error> {
    let certificate = &invitation.certificate;
    let holds = certificate.member == invitation.key.id()
        && matches!(
            certificate.kind,
            membership::MemberKind::Device | membership::MemberKind::Agent
        )
        && certificate.verify(&certificate.account_key);
    match holds {
        true => Ok(()),
        false => Err(Error::new(
            ErrorCode::InvalidCertificate,
            "the invitation's certificate does not hold",
        )),
    }
}

/// Whether the account of `invitation` is known: it is the database's
/// own, or one a person confirmed before, or `fingerprint` is its
/// fingerprint, which a person compared. Says whether the add confirms the
/// account, which the database then remembers.
fn known(
    inner: &Inner,
    invitation: &Invitation,
    fingerprint: Option<Fingerprint>,
) -> Result<bool, Error> {
    let account = invitation.fingerprint();
    let confirmed = inner
        .view
        .account_confirmed_at(&account.0)
        .map_err(storage)?
        .is_some();
    if account.0 == inner.account.verifying_key().to_bytes() || confirmed {
        return Ok(false);
    }
    if fingerprint == Some(account) {
        return Ok(true);
    }
    Err(Error::new(
        ErrorCode::UnknownAccount,
        "the invitation's account is new to this database: pass its fingerprint \
         once a person compared it",
    )
    .with_fingerprint(account))
}

/// Remembers that a person confirmed the account with the public key
/// `account`, now.
fn confirm(view: &mut View, account: &[u8; 32]) -> Result<(), Error> {
    view.confirm_account(account, crate::clock::now_millis())
        .map_err(storage)
}

/// The heads of the control DAG of `space` in `view`: the folded control
/// changes no other control change depends on. A control change depends
/// only on control changes (RFC 0008, "Terms"): the relay, which cannot
/// open sealed changes, checks it over the control DAG, and would take a
/// sealed dependency for a gap.
pub(crate) fn control_heads(view: &View, space: SpaceId) -> BTreeSet<ChangeId> {
    view.log(space)
        .map(|log| log.control_heads().clone())
        .unwrap_or_default()
}

#[cfg(test)]
mod batch_tests;
#[cfg(test)]
mod known_tests;
#[cfg(test)]
mod leave_tests;
#[cfg(test)]
mod list_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod relay_tests;
#[cfg(test)]
mod remove_tests;
#[cfg(test)]
mod rotate_tests;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod write_tests;
