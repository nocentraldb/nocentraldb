//! Removing a member and setting its role (RFC 0009, "Invitations and
//! members"; RFC 0008, "Membership entry", "The cutoff" and "A member
//! demoted and raised again"; RFC 0003, "Removing a member and rotating the
//! space key").
//!
//! A removal is a revocation of every right with a cutoff, answered at once
//! by a rotation whose envelopes leave the removed member out (eager
//! rotation, RFC 0003). Setting a role is a grant of the new role and a
//! revocation, with a cutoff, of every right above it; lowering a role does
//! not rotate, since every role reads the space. A set role that gives the
//! right to write back to a reader is a re-grant: the admin rotates first,
//! leaving the subject out, writes the grant from the fresh generation,
//! and only then the subject's envelope, so that no envelope to the subject
//! for that generation is in the grant's causal past.
//!
//! Each call writes all its changes into one [`Batch`], folded in one
//! transaction. When the call rotates, the rotation re-wraps every person
//! key of the space, and a space with a person key this member does not
//! hold is refused before anything is written, as [`rotate::rotatable`]
//! says.

use std::collections::BTreeSet;

use ncdb_core::envelope;
use ncdb_core::ids;
use ncdb_core::membership::{self, Action, Entry, MembershipEntry, Resolved};

use super::batch::Batch;
use super::{MemberId, Members, Notice, Role, holds, rotate};
use crate::db::{Inner, SpaceState, lock};
use crate::error::{Error, ErrorCode};
use crate::space::{not_stopped, writable};

impl Members<'_> {
    /// Removes `member` from the space: writes the removal, with the
    /// member's highest author sequence this copy holds as its cutoff
    /// (RFC 0008, "The cutoff"), then a rotation to a fresh generation that
    /// answers it, with an envelope for every remaining member and none for
    /// the removed one, and folds them in one transaction. The removed
    /// member keeps its copy of everything it received before, which the
    /// returned [`Notice`] says (`MemberRemoved`).
    ///
    /// Only an admin removes members: another member is refused with
    /// `NotAllowed`, a removed one with `NotMember`. A member that is not a
    /// member of the space is refused with `NotFound`, and the database's
    /// own member with `InvalidValue`, since a member that leaves must not
    /// know the next key (RFC 0008, "Membership entry"): leaving is
    /// `space.leave()`, which is not built. The rotation re-wraps every
    /// person key of the space under the fresh generation. A space with a
    /// person key this member does not hold, or one where it lacks the key
    /// of a generation the rotation chains, is refused with
    /// `InvalidValue`. Nothing is written when a
    /// call is refused.
    pub async fn remove(&self, member: MemberId) -> Result<Notice, Error> {
        let mut inner = lock(self.space.inner());
        let space = self.space.id();
        writable(&inner, space)?;
        not_stopped(&inner, space)?;
        let me = inner.member.key().id();
        let subject = member.0;
        {
            let resolved = inner.spaces[&space.0].checks.resolved();
            holds(resolved, me, membership::Role::Admin, "removes members")?;
            current(resolved, subject)?;
        }
        if subject == me {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "a member does not remove itself: leaving a space is space.leave()",
            ));
        }
        rotate::rotatable(&inner.spaces[&space.0])?;
        write(&mut inner, space.0, subject, |batch, state, key, cutoff| {
            let removal = batch.write(
                key,
                Entry::Membership(MembershipEntry {
                    subject,
                    action: Action::Remove { cutoff },
                }),
                None,
            );
            batch.heads = BTreeSet::from([removal]);
            rotate::rotate(
                batch,
                state,
                key,
                space.0,
                BTreeSet::from([removal]),
                Some(subject),
            );
        })?;
        Ok(Notice::MEMBER_REMOVED)
    }

    /// Sets the role of `member` to `role`: writes a set role, a grant of
    /// `role` and a revocation of every right above it, with the member's
    /// highest author sequence this copy holds as its cutoff. Lowering a
    /// role does not rotate. Giving the right to write to a reader is a
    /// re-grant (RFC 0008, "A member demoted and raised again"): the space
    /// key rotates first, the grant names the fresh generation, and the
    /// member's envelope follows it. Returns the [`Notice`] `RoleSet`: a
    /// member whose role was lowered keeps its copy of everything it
    /// received before now.
    ///
    /// Any member may lower its own role; only an admin raises a role or
    /// sets another member's, others are refused with `NotAllowed`, a
    /// removed member with `NotMember`. The last admin may not lower its
    /// own role, which is refused with `LastAdmin`. A member that is not a
    /// member of the space is refused with `NotFound`; a re-grant in a
    /// space with a person key this member does not hold with
    /// `InvalidValue`. Nothing is written
    /// when a call is refused.
    pub async fn set_role(&self, member: MemberId, role: Role) -> Result<Notice, Error> {
        let mut inner = lock(self.space.inner());
        let space = self.space.id();
        writable(&inner, space)?;
        not_stopped(&inner, space)?;
        let me = inner.member.key().id();
        let subject = member.0;
        let new: membership::Role = role.into();
        let regrant = {
            let resolved = inner.spaces[&space.0].checks.resolved();
            let was = current(resolved, subject)?;
            match subject == me && new <= was {
                true => holds(resolved, me, membership::Role::Reader, "lowers its role")?,
                false => holds(resolved, me, membership::Role::Admin, "sets roles")?,
            }
            if was == membership::Role::Admin && new < was && admins(resolved) == 1 {
                return Err(Error::new(
                    ErrorCode::LastAdmin,
                    "the space's last admin may not lower its own role",
                ));
            }
            new >= membership::Role::Writer && was < membership::Role::Writer
        };
        if regrant {
            rotate::rotatable(&inner.spaces[&space.0])?;
        }
        write(&mut inner, space.0, subject, |batch, state, key, cutoff| {
            if regrant {
                rotate::rotate(batch, state, key, space.0, BTreeSet::new(), Some(subject));
            }
            let writes = new >= membership::Role::Writer;
            let grant = batch.write(
                key,
                Entry::Membership(MembershipEntry {
                    subject,
                    action: Action::SetRole {
                        role: new,
                        from_generation: writes.then_some(batch.generation),
                        cutoff,
                    },
                }),
                None,
            );
            if regrant {
                let recipient = state
                    .checks
                    .resolved()
                    .member(subject)
                    .expect("the subject is a member")
                    .key;
                let sealed =
                    envelope::seal(batch.key(state), space.0, batch.generation, &recipient)
                        .expect("a member's public key takes an envelope");
                batch.write(
                    key,
                    Entry::KeyEnvelope(sealed),
                    Some(BTreeSet::from([grant])),
                );
            }
        })?;
        Ok(Notice::ROLE_SET)
    }
}

/// The role of `subject` in `resolved`, which must be a current member:
/// another is refused with `NotFound`.
fn current(resolved: &Resolved, subject: ids::MemberId) -> Result<membership::Role, Error> {
    resolved.role(subject).ok_or_else(|| {
        Error::new(
            ErrorCode::NotFound,
            "the member is not a member of the space",
        )
    })
}

/// How many admins `resolved` holds.
fn admins(resolved: &Resolved) -> usize {
    resolved
        .members()
        .filter(|(_, role)| *role == membership::Role::Admin)
        .count()
}

/// Writes one membership call about `subject` into `space`: `fill` adds
/// its changes to a batch, given the cutoff of a revocation of `subject`,
/// its highest author sequence the log holds, or 0 when it holds none (RFC
/// 0008, "The cutoff"). The batch is then folded in one transaction, the
/// person-key records a rotation re-wrapped are handed to syncing, and sync
/// is woken.
fn write(
    inner: &mut Inner,
    space: ids::SpaceId,
    subject: ids::MemberId,
    fill: impl FnOnce(&mut Batch, &SpaceState, &ncdb_core::keys::MemberKey, u64),
) -> Result<(), Error> {
    let key = inner.member.key();
    let Inner {
        view,
        spaces,
        member: secrets,
        ..
    } = inner;
    let state = spaces.get_mut(&space).expect("a writable space is held");
    let cutoff = view
        .tips(space, &state.checks)
        .map_or(0, |tips| tips.next_sequence(subject) - 1);
    let mut batch = Batch::new(view, state, &key, space);
    fill(&mut batch, state, &key, cutoff);
    let records = batch.commit(view, state, secrets)?;
    crate::sync::put_person_keys(inner, space, records);
    crate::sync::wake(inner);
    Ok(())
}
