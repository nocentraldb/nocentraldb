//! The unconfirmed changes a push carries, read a push at a time (RFC
//! 0007, "Island mode", step 2; NOC-519). A burst of writes that outpaces
//! the push leaves many changes of a space unconfirmed; the push reads at
//! most [`wire::MAX_PUSH`] of them at once, so the writer's memory does not
//! grow with the burst. Each read groups its changes into the pushes
//! [`wire::batches`] makes of all of them: a page holds back a last push
//! that more changes could still fill, so the pushes are the same as if the
//! push had read every change at once.

use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::view::Error as ViewError;

use super::{again, wire};
use crate::db::Inner;

/// The pushes of one read of a space's unconfirmed changes.
pub(super) struct Page {
    /// The pushes to send, in local order: the ids of their changes and
    /// their frames.
    pub(super) pushes: Vec<(Vec<ChangeId>, Vec<Vec<u8>>)>,
    /// The local position of the last change the pushes carry, after
    /// which the next read starts.
    pub(super) after: u64,
    /// Whether the push ends with these pushes.
    pub(super) last: bool,
}

/// Where a push of `space` that starts now ends: the local position of
/// the newest change its log holds. A change written during the push waits
/// for the next one. A refused change the fold newly accepted since, after
/// a control change the member wrote itself, is unconfirmed again first
/// (RFC 0008, "How the relay applies the rules").
pub(super) fn through(inner: &mut Inner, space: SpaceId) -> Result<u64, ViewError> {
    again::check(inner, space)?;
    inner.view.last_local_position(space)
}

/// The next pushes of `space`: those of the changes the relay has neither
/// confirmed nor refused, in local order, after the local position `after`
/// and up to `through`, this member's own and those it received from
/// others. It reads at most [`wire::MAX_PUSH`] changes.
pub(super) fn next(
    inner: &mut Inner,
    space: SpaceId,
    after: u64,
    through: u64,
) -> Result<Page, ViewError> {
    let read = inner
        .view
        .unconfirmed_between(space, after, through, wire::MAX_PUSH)?;
    let last = read.len() < wire::MAX_PUSH;
    let mut positions = Vec::with_capacity(read.len());
    let mut ids = Vec::with_capacity(read.len());
    let mut frames = Vec::with_capacity(read.len());
    for (position, change) in read {
        positions.push(position);
        ids.push(change.id());
        frames.push(change.bytes().to_vec());
    }
    let mut batches = wire::batches(frames);
    // A last push that is not full could take changes of the next read:
    // it waits for that read, unless it is the only one.
    if !last && batches.len() > 1 && batches.last().is_some_and(|b| b.len() < wire::MAX_PUSH) {
        batches.pop();
    }
    let mut ids = ids.into_iter();
    let mut pushes = Vec::with_capacity(batches.len());
    let mut carried = 0;
    for batch in batches {
        carried += batch.len();
        pushes.push((ids.by_ref().take(batch.len()).collect(), batch));
    }
    let after = match carried {
        0 => after,
        n => positions[n - 1],
    };
    Ok(Page {
        pushes,
        after,
        last,
    })
}
