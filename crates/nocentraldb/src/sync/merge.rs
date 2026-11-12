//! When an admin's SDK writes a merge rotation (RFC 0008, "Two removals
//! that rotate the key at once", step 5; NOC-220): after each pass of the
//! session over the spaces, every space where this member is an admin and
//! the current generation is exposed gets a deadline, a random delay of up
//! to [`SyncState::merge_delay`](super::SyncState) away, a minute unless a
//! test changes it; a space no longer exposed loses its deadline. When a
//! deadline passes, the session checks again and writes the rotation
//! (`members::merge`). Another admin's merge rotation, folded before the
//! deadline, ends the exposure and so cancels this one. Two that cross are
//! concurrent rotations, and step 5 repeats (step 6).

use std::time::Duration;

use super::time::Instant;

use ncdb_core::ids::SpaceId;

use crate::db::Inner;
use crate::members::merge;

/// Sets or drops the deadline of every space, as of `now`, and returns the
/// spaces whose deadline passed, which lose it.
pub(super) fn due(inner: &mut Inner, now: Instant) -> Vec<SpaceId> {
    let wanted: Vec<SpaceId> = inner
        .spaces
        .iter()
        .filter(|(space, _)| !super::stopped(inner, **space))
        .filter(|(_, state)| merge::wanted(inner, state))
        .map(|(space, _)| *space)
        .collect();
    let Some(sync) = inner.sync.as_mut() else {
        return Vec::new();
    };
    sync.merges.retain(|space, _| wanted.contains(space));
    for space in wanted {
        let delay = sync.merge_delay;
        sync.merges
            .entry(space)
            .or_insert_with(|| now + random_up_to(delay));
    }
    let due: Vec<SpaceId> = sync
        .merges
        .iter()
        .filter(|(_, deadline)| **deadline <= now)
        .map(|(space, _)| *space)
        .collect();
    for space in &due {
        sync.merges.remove(space);
    }
    due
}

/// The earliest deadline, if any.
pub(super) fn next(inner: &Inner) -> Option<Instant> {
    inner.sync.as_ref()?.merges.values().min().copied()
}

/// Writes the merge rotation of `space`, if it is still wanted. A space
/// this member cannot rotate yet gets a new deadline at the next pass.
pub(super) fn write(inner: &mut Inner, space: SpaceId) {
    let _ = merge::merge(inner, space);
}

/// A random delay from zero up to `longest`, from the platform's secure
/// generator; zero when it fails.
fn random_up_to(longest: Duration) -> Duration {
    let millis = u64::try_from(longest.as_millis()).unwrap_or(u64::MAX);
    if millis == 0 {
        return Duration::ZERO;
    }
    let mut bytes = [0u8; 8];
    if getrandom::fill(&mut bytes).is_err() {
        return Duration::ZERO;
    }
    Duration::from_millis(u64::from_le_bytes(bytes) % (millis + 1))
}

/// Sets the longest delay before `db` writes a merge rotation, and draws
/// every pending deadline again under it.
#[cfg(test)]
pub(crate) fn set_merge_delay(db: &crate::Db, delay: Duration) {
    let mut inner = crate::db::lock(&db.inner);
    if let Some(sync) = inner.sync.as_mut() {
        sync.merge_delay = delay;
        sync.merges.clear();
    }
    super::wake(&inner);
}
