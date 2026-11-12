//! The finalize an admin's SDK writes on its own (RFC 0008, "A provisional
//! erasure in a space with one account"; Carsten, 2026-11-11, NOC-230): once
//! this device has held a provisional shred for 7 days by its own clock,
//! an admin of the space that belongs to the performer's account writes an
//! erasure decision that finalizes it. When to write it, and the push again
//! after a `too_early`, are syncing's (`sync::finalize`). The SDK writes no
//! take-back: RFC 0009 names no call for it yet.

use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::membership::{Entry, ErasureDecision, Role};

use crate::db::{Inner, SpaceState};
use crate::error::Error;
use crate::space::{not_stopped, writable};

use super::change::write;

/// The provisional shreds of `state`'s space that this member may
/// finalize, with when this device first held each: it is an admin of the
/// space and belongs to the performer's account, and no accepted decision
/// ended the shred.
pub(crate) fn finalizable(
    inner: &Inner,
    space: SpaceId,
    state: &SpaceState,
) -> Vec<(ChangeId, Option<u64>)> {
    let resolved = state.checks.resolved();
    let me = inner.member.key().id();
    if resolved.role(me) != Some(Role::Admin) {
        return Vec::new();
    }
    crate::status::pending(resolved)
        .filter(|(_, performer)| resolved.same_account(me, *performer))
        .map(|(shred, _)| (shred, inner.view.shred_held_at(space, shred).ok().flatten()))
        .collect()
}

/// Writes the erasure decision that finalizes `shred` in `space`, if this
/// member may still finalize it. Whether it wrote one.
pub(crate) fn finalize(inner: &mut Inner, space: SpaceId, shred: ChangeId) -> Result<bool, Error> {
    let id = crate::id::SpaceId(space);
    writable(inner, id)?;
    not_stopped(inner, id)?;
    let state = &inner.spaces[&space];
    if !finalizable(inner, space, state)
        .iter()
        .any(|(pending, _)| *pending == shred)
    {
        return Ok(false);
    }
    let me = inner.member.key().id();
    write(inner, space, me, |batch, _, key, _| {
        let entry = Entry::ErasureDecision(ErasureDecision {
            shred,
            finalize: true,
        });
        batch.write(key, entry, None);
    })?;
    Ok(true)
}
