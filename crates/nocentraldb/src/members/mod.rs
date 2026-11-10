//! The members of a space (RFC 0009, "Invitations and members"): an admin
//! adds a member by its invitation, which writes a membership entry and a
//! key envelope of the current generation's space key to the new member
//! (RFC 0008, "Membership entry" and "Key envelope"; RFC 0003, "Sharing a
//! space key with a new member").
//!
//! **What this does not do yet.** `list`, `set_role` and `remove`, adding
//! a list of invitations, and the notice RFC 0009 has `add` return.

use std::collections::BTreeSet;

use ncdb_core::change::Kind;
use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::log::{Position, State};
use ncdb_core::membership::{self, Action, ControlBody, Entry, MembershipEntry};
use ncdb_core::view::View;

use crate::db::{Inner, lock};
use crate::error::Error;
use crate::next;
use crate::space::{Space, fold, writable};

mod invitation;

pub use invitation::{Fingerprint, Invitation};

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
    /// reads the whole history of the space once it syncs.
    pub async fn add(&self, invitation: &Invitation, options: AddMember) -> Result<(), Error> {
        let mut inner = lock(self.space.inner());
        let space = self.space.id();
        writable(&inner, space)?;
        let member = inner.member.key();
        let Inner { view, spaces, .. } = &mut *inner;
        let state = spaces.get_mut(&space.0).expect("a writable space is held");
        let next = next::next(view, space.0, &state.checks, member.id());
        let generation = next.generation.expect("a writable space has a generation");
        let subject = invitation.key.id();
        let added = membership::write(
            &member,
            space.0,
            next.sequence,
            generation,
            &ControlBody::new(
                control_heads(view, space.0),
                Entry::Membership(MembershipEntry {
                    subject,
                    action: Action::Add {
                        role: options.role.into(),
                        key: invitation.key,
                        certificate: Box::new(invitation.certificate.clone()),
                        from_generation: generation,
                    },
                }),
            ),
        );
        let key = state
            .checks
            .space_key(generation)
            .expect("a writable space's key is held");
        let sealed = envelope::seal(key, space.0, generation, &invitation.key)
            .expect("a member's public key takes an envelope");
        let sent = membership::write(
            &member,
            space.0,
            next.sequence + 1,
            generation,
            &ControlBody::new(BTreeSet::from([added.id()]), Entry::KeyEnvelope(sealed)),
        );
        for change in [&added, &sent] {
            fold(view, &mut state.checks, change)?;
        }
        crate::sync::wake(&inner);
        Ok(())
    }
}

/// The heads of the control DAG of `space` in `view`: the folded control
/// changes no other control change depends on. A control change depends
/// only on control changes (RFC 0008, "Terms"): the relay, which cannot
/// open sealed changes, checks it over the control DAG, and would take a
/// sealed dependency for a gap.
fn control_heads(view: &View, space: SpaceId) -> BTreeSet<ChangeId> {
    let mut heads = BTreeSet::new();
    let Some(log) = view.log(space) else {
        return heads;
    };
    let mut covered = BTreeSet::new();
    for (_, change) in log.after(Position::START) {
        let folded = matches!(log.state(change.id()), Some(State::Folded { .. }));
        if change.header().kind != Kind::CONTROL || !folded {
            continue;
        }
        if let Ok(body) = ControlBody::decode(&change.header().payload) {
            covered.extend(body.dependencies);
        }
        heads.insert(change.id());
    }
    heads.retain(|id| !covered.contains(id));
    heads
}

#[cfg(test)]
mod tests;
