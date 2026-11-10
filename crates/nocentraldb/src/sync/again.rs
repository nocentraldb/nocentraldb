//! Pushing again a change the relay refused (RFC 0008, M19; RFC 0002, "A
//! change the relay refuses after the member folded it"). A member does
//! not push a refused change to the relay again, except when its own fold
//! newly accepts it after a control change arrived: a concurrent change
//! may have made the revocation behind the refusal ineffective.
//!
//! The sync state keeps each refused change and whether the fold has
//! rejected it since the refusal. When a rejected one is folded again, its
//! refusal is cleared, so it is unconfirmed and pushed once more. If the
//! relay refuses it again, the refusal is recorded again, and only another
//! newly accepting fold pushes it: nothing loops. The rejections seen are
//! kept in memory; a database that opens takes those its log holds.

use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::log::State;
use ncdb_core::view::Error as ViewError;

use crate::db::Inner;

/// Whether the member's fold rejects the change `id` of `space`.
fn rejected(inner: &Inner, space: SpaceId, id: ChangeId) -> bool {
    matches!(inner.view.state(space, id), Some(State::Rejected(_)))
}

/// Takes the refused changes of every space the database holds, as the
/// log has them when the database opens.
pub(super) fn load(inner: &mut Inner) -> Result<(), ViewError> {
    let spaces: Vec<SpaceId> = inner.spaces.keys().copied().collect();
    for space in spaces {
        for (change, _) in inner.view.refused(space)? {
            refused(inner, space, change.id());
        }
    }
    Ok(())
}

/// Keeps the change `id` of `space`, which the relay just refused.
pub(super) fn refused(inner: &mut Inner, space: SpaceId, id: ChangeId) {
    let seen = rejected(inner, space, id);
    if let Some(sync) = inner.sync.as_mut() {
        sync.refused.entry(space).or_default().insert(id, seen);
    }
}

/// Checks the refused changes of `space` after the fold may have changed
/// their state. A rejected one is marked; one folded again after it was
/// rejected loses its refusal, to be pushed once more. Returns whether any
/// did.
pub(super) fn check(inner: &mut Inner, space: SpaceId) -> Result<bool, ViewError> {
    let Some(kept) = inner.sync.as_ref().and_then(|s| s.refused.get(&space)) else {
        return Ok(false);
    };
    let mut seen = Vec::new();
    let mut accepted = Vec::new();
    for (&id, &was_rejected) in kept {
        match inner.view.state(space, id) {
            Some(State::Rejected(_)) if !was_rejected => seen.push(id),
            Some(State::Folded { .. }) if was_rejected => accepted.push(id),
            _ => {}
        }
    }
    for id in &accepted {
        inner.view.clear_refusal(space, *id)?;
    }
    let kept = inner
        .sync
        .as_mut()
        .and_then(|s| s.refused.get_mut(&space))
        .expect("the refused changes of the space");
    for id in seen {
        kept.insert(id, true);
    }
    for id in &accepted {
        kept.remove(id);
    }
    Ok(!accepted.is_empty())
}
