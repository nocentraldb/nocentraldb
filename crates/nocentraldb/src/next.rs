//! What the member's next change in a space takes from its log (RFC 0002,
//! "The change" and "Dependencies"): the generation it seals under, its
//! author sequence, the heads it depends on and the highest clock seen.
//! The log is the one source, so a space the member created and one it
//! joined through sync are written into alike, and a change pulled since
//! the member's last write is among the heads of its next one.

use std::collections::BTreeSet;

use ncdb_core::change::Kind;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::log::{Position, State};
use ncdb_core::membership::ControlBody;
use ncdb_core::receive::Checks;
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
/// change in its payload, which the checks open. A change the log holds
/// but did not fold is no head: a rejected or parked sealed change cannot
/// be read, so the changes it depends on are not known.
///
/// It reads and opens the whole log on each write; see the decision log
/// (NOC-119).
pub(crate) fn next(view: &View, space: SpaceId, checks: &SealChecks, me: MemberId) -> Next {
    let mut next = Next {
        generation: checks.resolved().current_generation(),
        sequence: 1,
        heads: BTreeSet::new(),
        seen: Hlc::default(),
    };
    let Some(log) = view.log(space) else {
        return next;
    };
    let mut covered = BTreeSet::new();
    for (_, change) in log.after(Position::START) {
        let header = change.header();
        if header.author == me {
            next.sequence = next.sequence.max(header.sequence + 1);
        }
        if !matches!(log.state(change.id()), Some(State::Folded { .. })) {
            continue;
        }
        let dependencies = match header.kind {
            Kind::CONTROL => ControlBody::decode(&header.payload).map(|body| body.dependencies),
            _ => match checks.open(change) {
                Ok(payload) => {
                    next.seen = next.seen.max(payload.clock);
                    Ok(payload.dependencies)
                }
                Err(_) => continue,
            },
        };
        covered.extend(dependencies.unwrap_or_default());
        next.heads.insert(change.id());
    }
    next.heads.retain(|id| !covered.contains(id));
    next
}
