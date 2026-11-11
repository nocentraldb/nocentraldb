//! What the member's next change in a space takes from its log (RFC 0002,
//! "The change" and "Dependencies"): the generation it seals under, its
//! author sequence, the heads it depends on and the highest clock seen.
//! The log is the one source, so a space the member created and one it
//! joined through sync are written into alike, and a change pulled since
//! the member's last write is among the heads of its next one.
//!
//! The log keeps its tips as changes are folded, written or pulled, and
//! un-folded by a revocation ([`ncdb_core::tips`]), so a write reads them
//! without a pass over the log. After the database opens, the first write
//! into a space opens each of its folded sealed changes once, since the
//! view reads their states back without their payloads (NOC-138).

use std::collections::BTreeSet;

use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::view::View;

/// What the member's next change in a space takes from the log.
pub(crate) struct Next {
    /// The current generation (RFC 0008), which a writer seals under.
    pub(crate) generation: Option<u64>,
    /// The member's next author sequence: one more than the highest of its
    /// changes the log holds.
    pub(crate) sequence: u64,
    /// The heads: the folded changes no other folded change depends on.
    pub(crate) heads: BTreeSet<ChangeId>,
    /// The highest clock among the folded sealed changes.
    pub(crate) seen: Hlc,
}

/// What the member `me`'s next change in `space` takes from the log of
/// that space in `view`, read with the space's `checks`.
///
/// The heads are taken among the folded changes, whose dependencies the
/// member can read: a control change names them in its body, a sealed
/// change in its payload. A change the log holds but did not fold is no
/// head: a rejected or parked sealed change cannot be read, so the changes
/// it depends on are not known.
pub(crate) fn next(view: &mut View, space: SpaceId, checks: &SealChecks, me: MemberId) -> Next {
    let generation = checks.resolved().current_generation();
    match view.tips(space, checks) {
        Some(tips) => Next {
            generation,
            sequence: tips.next_sequence(me),
            heads: tips.heads().clone(),
            seen: tips.seen(),
        },
        None => Next {
            generation,
            sequence: 1,
            heads: BTreeSet::new(),
            seen: Hlc::default(),
        },
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "next_tests.rs"]
mod tests;
