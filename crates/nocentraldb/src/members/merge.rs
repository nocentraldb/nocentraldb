//! The merge rotation an admin's SDK writes on its own (RFC 0008, "Two
//! removals that rotate the key at once", step 5; NOC-220): when the
//! current generation is exposed, as after a member left or a removal
//! whose author never rotated, a key chain entry that lists every head of
//! the generation graph, with envelopes for the current members only. It is
//! [`rotate::rotate`] with no member left out, written through the same
//! batch, commit, person-key put and wake as an add or a removal. When to
//! write it is syncing's (`sync::merge`).
//!
//! A merge rotation answers no revocation: its `answers` is empty, as the
//! core's `MergeRotation::entry` writes it, the one after a leave included.

use std::collections::BTreeSet;

use ncdb_core::ids::SpaceId;
use ncdb_core::membership::Role;

use crate::db::{Inner, SpaceState};
use crate::error::Error;
use crate::space::{not_stopped, writable};

use super::change::write;
use super::rotate;

/// Whether this member may write a merge rotation of `state`'s space now:
/// it is an admin, and the space's current generation is exposed.
pub(crate) fn wanted(inner: &Inner, state: &SpaceState) -> bool {
    let resolved = state.checks.resolved();
    resolved.role(inner.member.key().id()) == Some(Role::Admin)
        && resolved
            .current_generation()
            .is_some_and(|generation| resolved.exposed(generation))
}

/// Writes a merge rotation of `space` if it is still wanted. Whether it
/// wrote one. A space this member cannot rotate yet, because it holds no
/// key of a head or of a live person key, is an error, and nothing is
/// written.
pub(crate) fn merge(inner: &mut Inner, space: SpaceId) -> Result<bool, Error> {
    let id = crate::id::SpaceId(space);
    writable(inner, id)?;
    not_stopped(inner, id)?;
    let state = &inner.spaces[&space];
    if !wanted(inner, state) {
        return Ok(false);
    }
    rotate::rotatable(state)?;
    let me = inner.member.key().id();
    write(inner, space, me, |batch, state, key, _| {
        rotate::rotate(batch, state, key, space, BTreeSet::new(), None);
    })?;
    Ok(true)
}
