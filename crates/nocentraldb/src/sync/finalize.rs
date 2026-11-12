//! When a device of the performer's account finalizes a provisional
//! erasure, and the push again after a `too_early` (RFC 0008, "A provisional
//! erasure in a space with one account"; RFC 0007, "What the relay
//! checks"; pm, NOC-234). After each pass of the session over the spaces,
//! every provisional shred this member may finalize gets a deadline: 7 days
//! after this device first held it, by its own clock (the view's
//! `shreds_held`). When a deadline passes, the session writes the finalize
//! (`members::erasure`) and pushes it.
//!
//! The relay counts the 7 days from when it accepted the shred, by its own
//! clock, and refuses a finalize that comes sooner with `too_early`. The
//! device cannot read that clock, so a `too_early` is never recorded as a
//! refusal: the finalize stays unconfirmed, and the space is pushed again
//! an hour later, until the relay accepts it.

use std::time::{Duration, Instant};

use ncdb_core::ids::{ChangeId, SpaceId};

use crate::db::Inner;
use crate::members::erasure;

/// The provisional shreds whose 7 days passed on this device, as of `now`,
/// by space.
pub(super) fn due(inner: &Inner, now: u64) -> Vec<(SpaceId, ChangeId)> {
    let Some(sync) = inner.sync.as_ref() else {
        return Vec::new();
    };
    let after = millis(sync.finalize_after);
    inner
        .spaces
        .iter()
        .filter(|(space, _)| !super::stopped(inner, **space))
        .flat_map(|(space, state)| {
            erasure::finalizable(inner, *space, state)
                .into_iter()
                .filter(move |(_, held_at)| {
                    held_at.is_some_and(|at| at.saturating_add(after) <= now)
                })
                .map(move |(shred, _)| (*space, shred))
        })
        .collect()
}

/// The earliest moment a finalize or a push again is due, if any: `now` is
/// the device's clock in milliseconds, and `instant` the same moment.
pub(super) fn next(inner: &Inner, now: u64, instant: Instant) -> Option<Instant> {
    let sync = inner.sync.as_ref()?;
    let after = millis(sync.finalize_after);
    let finalizes = inner.spaces.iter().flat_map(|(space, state)| {
        erasure::finalizable(inner, *space, state)
            .into_iter()
            .filter_map(|(_, held_at)| held_at)
            .map(move |at| {
                instant + Duration::from_millis(at.saturating_add(after).saturating_sub(now))
            })
    });
    finalizes.chain(sync.retries.values().copied()).min()
}

/// Writes the finalize of `shred` in `space`, if it is still wanted. The
/// write wakes the session, which pushes it.
pub(super) fn write(inner: &mut Inner, space: SpaceId, shred: ChangeId) {
    let _ = erasure::finalize(inner, space, shred);
}

/// The spaces whose push again is due as of `now`, which lose their
/// deadline.
pub(super) fn retries_due(inner: &mut Inner, now: Instant) -> Vec<SpaceId> {
    let Some(sync) = inner.sync.as_mut() else {
        return Vec::new();
    };
    let due: Vec<SpaceId> = sync
        .retries
        .iter()
        .filter(|(_, at)| **at <= now)
        .map(|(space, _)| *space)
        .collect();
    for space in &due {
        sync.retries.remove(space);
    }
    due
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Sets how long `db` holds a provisional shred before it finalizes it, and
/// how long after a `too_early` it pushes again, as the tests need them.
#[cfg(test)]
pub(crate) fn set_finalize_after(db: &crate::Db, after: Duration, retry: Duration) {
    let mut inner = crate::db::lock(&db.inner);
    if let Some(sync) = inner.sync.as_mut() {
        sync.finalize_after = after;
        sync.too_early_retry = retry;
        sync.retries.clear();
    }
    super::wake(&inner);
}
