//! Removing a lost member of the account (RFC 0010, "Losing a device";
//! NOC-268), the account space's side. The device that runs it removes the
//! lost member from every space it administers where the account space
//! records the member, which rotates each space key, and lists the spaces
//! where no device of the account is an admin, for the app to ask their
//! admins. Then it marks the member's record removed and removes the
//! member from the account space, which rotates its key too.
//!
//! A space that another device of the account administers, and this one
//! does not, is in neither list: that device removes the member when it
//! reads the record that says `removed` ([`super::reconcile`]).

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::ids::{self, SpaceId as CoreSpaceId};
use ncdb_core::membership::Role as CoreRole;

use super::{Kind, MEMBERS, member_object, member_rows, memberships, model, space};
use crate::db::Inner;
#[cfg(test)]
use crate::db::{Db, lock};
use crate::error::{Error, ErrorCode};
use crate::id::{ObjectId, SpaceId};
#[cfg(test)]
use crate::members::MemberId;
use crate::members::{Notice, Role, remove_member};
use crate::space::{not_stopped, writable_or_account};
use crate::tx::Transaction;
use crate::value::Value;

/// What removing a lost member did.
#[derive(Debug)]
pub(crate) struct Removal {
    /// The spaces this device removed the member from, each with
    /// `MemberRemoved`'s notice, in the order of their ids.
    pub(crate) removed: Vec<(SpaceId, Notice)>,
    /// The spaces where the member is a member and no device of the
    /// account is an admin, whose admins the app asks, in the order of
    /// their ids.
    pub(crate) ask_admins: Vec<SpaceId>,
}

#[cfg(test)]
impl Db {
    /// [`remove_lost`] under the database's lock.
    pub(crate) async fn remove_lost(&self, member: MemberId) -> Result<Removal, Error> {
        remove_lost(&mut lock(&self.inner), member.0)
    }
}

/// Removes the lost member `lost` of the account, as the module says. A
/// member the account space holds no record of is refused with
/// `NotFound`, the database's own member with `InvalidValue`; so is a
/// device without an account space, which records no member. A space
/// this device would remove the member from but cannot rotate, since it
/// lacks a person key or a generation the rotation chains, refuses the
/// call with `InvalidValue` before anything is written.
pub(super) fn remove_lost(inner: &mut Inner, lost: ids::MemberId) -> Result<Removal, Error> {
    check(inner, lost)?;
    let account = space(inner).expect("checked");
    let plan = plan(inner, lost)?;
    let mut removed = Vec::new();
    for space in plan.remove {
        let notice = remove_member(inner, space, lost)?;
        removed.push((SpaceId(space), notice));
    }
    mark_removed(inner, account, lost)?;
    if held_member(inner, account, lost) {
        remove_member(inner, account, lost)?;
    }
    Ok(Removal {
        removed,
        ask_admins: plan.ask_admins.into_iter().map(SpaceId).collect(),
    })
}

/// Whether [`remove_lost`] would remove `lost`: refuses what it refuses,
/// and writes nothing.
pub(super) fn check(inner: &Inner, lost: ids::MemberId) -> Result<(), Error> {
    if lost == inner.member.key().id() {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "a device does not remove itself as lost",
        ));
    }
    let Some(account) = space(inner) else {
        return Err(not_recorded());
    };
    if !member_rows(inner)?.values().any(|m| m.member == lost.0) {
        return Err(not_recorded());
    }
    for space in plan(inner, lost)?.remove.iter().chain([&account]) {
        if held_member(inner, *space, lost) {
            writable_or_account(inner, SpaceId(*space))?;
            not_stopped(inner, SpaceId(*space))?;
            crate::members::rotatable(&inner.spaces[space])?;
        }
    }
    Ok(())
}

fn not_recorded() -> Error {
    Error::new(
        ErrorCode::NotFound,
        "the account space records no such member of the account",
    )
}

/// Which spaces this device removes the lost member from, and which it
/// lists for their admins.
struct Plan {
    remove: Vec<CoreSpaceId>,
    ask_admins: Vec<CoreSpaceId>,
}

/// The spaces the account space records the lost member in: those this
/// device administers and where the member is still a member, to remove
/// it from; and those where no other device of the account is recorded
/// as an admin and this device is none, to list.
fn plan(inner: &Inner, lost: ids::MemberId) -> Result<Plan, Error> {
    let me = inner.member.key().id();
    let devices: BTreeSet<[u8; 32]> = member_rows(inner)?
        .into_values()
        .filter(|m| m.kind == Kind::Device && !m.removed && m.member != lost.0)
        .map(|m| m.member)
        .collect();
    let mut admins: BTreeMap<[u8; 16], bool> = BTreeMap::new();
    let mut spaces = BTreeSet::new();
    for record in memberships(inner)? {
        if record.member == lost.0 {
            spaces.insert(record.space);
        } else if record.role == Role::Admin && devices.contains(&record.member) {
            admins.insert(record.space, true);
        }
    }
    let mut plan = Plan {
        remove: Vec::new(),
        ask_admins: Vec::new(),
    };
    for space in spaces.into_iter().map(CoreSpaceId) {
        let administers = inner
            .spaces
            .get(&space)
            .is_some_and(|state| state.checks.resolved().role(me) == Some(CoreRole::Admin));
        if administers {
            if held_member(inner, space, lost) {
                plan.remove.push(space);
            }
        } else if !admins.contains_key(&space.0) {
            plan.ask_admins.push(space);
        }
    }
    Ok(plan)
}

/// Whether this copy holds `space` with `member` a current member.
fn held_member(inner: &Inner, space: CoreSpaceId, member: ids::MemberId) -> bool {
    inner
        .spaces
        .get(&space)
        .is_some_and(|state| state.checks.resolved().role(member).is_some())
}

/// Marks the member record of `lost` removed, unless it already is.
fn mark_removed(inner: &mut Inner, account: CoreSpaceId, lost: ids::MemberId) -> Result<(), Error> {
    let object = member_object(lost.0);
    let held = member_rows(inner)?.get(&object).map(|m| m.removed);
    if held != Some(false) {
        return Ok(());
    }
    let mut tx = Transaction::reserved(model());
    tx.update(MEMBERS, ObjectId(object), [("removed", Value::from(true))])?;
    super::write(inner, account, tx).map(drop)
}

/// Removes each member whose record in the account space says `removed`
/// from every app space this device administers where it is still a
/// member, which rotates each key: the spaces the device that ran the
/// removal is not in. A space this device cannot write into or rotate
/// keeps the member, and the next reconcile tries again.
pub(super) fn remove_marked(inner: &mut Inner) -> Result<(), Error> {
    let me = inner.member.key().id();
    let marked: Vec<ids::MemberId> = member_rows(inner)?
        .into_values()
        .filter(|m| m.removed && m.member != me.0)
        .map(|m| ids::MemberId(m.member))
        .collect();
    if marked.is_empty() {
        return Ok(());
    }
    let app = inner.model_id;
    let held: Vec<CoreSpaceId> = inner
        .spaces
        .iter()
        .filter(|(_, state)| state.checks.model_id() == Some(app))
        .filter(|(_, state)| state.checks.resolved().role(me) == Some(CoreRole::Admin))
        .map(|(space, _)| *space)
        .collect();
    for space in held {
        for lost in &marked {
            if !held_member(inner, space, *lost)
                || not_stopped(inner, SpaceId(space)).is_err()
                || crate::members::rotatable(&inner.spaces[&space]).is_err()
            {
                continue;
            }
            let _ = remove_member(inner, space, *lost);
        }
    }
    Ok(())
}
