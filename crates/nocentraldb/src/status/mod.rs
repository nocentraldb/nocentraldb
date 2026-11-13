//! The status of one space (RFC 0009, "Sync and status": `space.status`):
//! everything a space holds that an app does not see in a read, with the
//! Rust names the owner decided on NOC-193. `space.status()` gives a
//! [`SpaceStatusReport`], whose `current()` gives a [`SpaceStatus`] and
//! whose `subscribe()` follows it.
//!
//! It reports no waiting shreds: a shred is final when accepted, so none
//! waits (RFC 0008, "A shred is final when accepted"; RFC 0009, "Sync and
//! status"). It reports the provisional erasures of a space with one
//! account instead, each with its age on this device, which counts from
//! when this device first held the shred: the view keeps that time
//! ([`note_shreds`]; the owner's NOC-218 A). The struct is
//! `#[non_exhaustive]`, so a field added later breaks no app.

mod reasons;

use std::collections::BTreeMap;
use std::time::Duration;

use futures_core::Stream;
use ncdb_core::ids::SpaceId as CoreSpaceId;
use ncdb_core::membership::{Resolved, Role as CoreRole, Verdict};
use ncdb_core::view::{Error as ViewError, View};

use crate::db::Inner;
use crate::id::ChangeId;
use crate::members::{Fingerprint, MemberId, Notice, Role};
use crate::space::Space;

pub use reasons::{ParkReason, RejectReason};

/// The status report of one space, from [`Space::status`].
#[derive(Debug)]
pub struct SpaceStatusReport<'a> {
    space: &'a Space,
}

/// What [`SpaceStatusReport::current`] gives: what the member's copy of
/// the space holds that a read does not show.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpaceStatus {
    /// This member's role in the space, or `None` once it was removed or
    /// while no membership of it has been folded yet, as in
    /// [`SpaceInfo`](crate::SpaceInfo). A member the relay answered
    /// `not_member` or `removed_member` for the space is removed. A
    /// removed member keeps reading its copy.
    pub role: Option<Role>,
    /// Whether a pull reached the relay's head in this relay epoch since
    /// the database opened; `false` without a relay.
    pub initial_sync_complete: bool,
    /// How many relay positions the member heard of but has not folded.
    pub behind: u64,
    /// The control dependencies the copy does not hold (RFC 0002,
    /// "Dependencies").
    pub gaps: Vec<ChangeId>,
    /// The parked changes and operations, per reason; the changes of
    /// another model apart, as [`ParkReason::AnotherModel`].
    pub parked: BTreeMap<ParkReason, u64>,
    /// The highest version of the app's own model that the parked changes
    /// and operations need, from each one's payload key 64 (RFC 0002, "What
    /// the model version is for"): an app that sees it above its own
    /// version can say "Some data needs a newer version of this app".
    /// `None` when no parked data names a version of this model; changes
    /// of another model never count. A version above `u32::MAX` reads as
    /// `u32::MAX`.
    pub newest_model: Option<u32>,
    /// The rejected changes, per reason.
    pub rejected: BTreeMap<RejectReason, u64>,
    /// This member's changes in this space the relay refused, in local
    /// order. They stay folded on this member and do not reach members
    /// that sync only through the relay (RFC 0002, D12).
    pub refused: Vec<Refused>,
    /// The accepted grants that a concurrent revocation of their subject
    /// capped, so that they stay accepted without effect (RFC 0008,
    /// "Concurrent add and remove of one member"). A grant that a later
    /// revocation revoked after seeing it is not lost.
    pub lost_grants: Vec<ChangeId>,
    /// The changes of every equivocation in the space, in local order: two
    /// or more different changes of one author under one sequence, as a
    /// device restored from a backup writes (RFC 0002, "Author
    /// sequence"; RFC 0008, "The cutoff"). Both are kept and folded.
    pub equivocations: Vec<ChangeId>,
    /// Whether the space's current key generation may be held by someone
    /// who is no longer a member (RFC 0008, "Two removals that rotate the
    /// key at once").
    pub exposed: bool,
    /// Whether this member is a member but holds no key for the space's
    /// current key generation, and waits for its envelope.
    pub waiting_for_key: bool,
    /// The accounts whose person-key records failed their checks when the
    /// member last fetched the space's records, in key order: their writer
    /// signed them, but they do not unwrap under their generation or their
    /// key does not match the commitment in the log (RFC 0003, "The person
    /// key"; the owner decided the name on NOC-222). The member ignores
    /// such a record.
    pub bad_person_key_records: Vec<Fingerprint>,
    /// The erasures of the space that are still provisional, in change id
    /// order: in a space whose members all belong to one account, the
    /// performer's own account signed the shred off, and no finalize or
    /// take-back is accepted yet (RFC 0008, "A provisional erasure in a
    /// space with one account"; Carsten, 2026-11-11, NOC-230). Every device
    /// of the account sees them, and an app must show them.
    pub provisional_erasures: Vec<ProvisionalErasure>,
}

/// A provisional erasure of a space with one account (RFC 0009, "Sync and
/// status").
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProvisionalErasure {
    /// The shred entry.
    pub change: ChangeId,
    /// How long ago this device first held the shred.
    pub age: Duration,
    /// Whether this device may take the erasure back: it is an admin of
    /// the space and belongs to the performer's account.
    pub can_undo: bool,
    /// The member that wrote the shred, so that an app can show which
    /// device erased (pm, 2026-11-12, NOC-234).
    pub performer: MemberId,
    /// The notice [`NoticeCode::ErasurePending`](crate::NoticeCode).
    pub notice: Notice,
}

/// One of this member's changes the relay refused.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Refused {
    /// The change the relay refused.
    pub change: ChangeId,
    /// The relay's code: one RFC 0007 lists, or `unknown_error` for any
    /// other text (NOC-188), never the relay's own text.
    pub code: String,
}

impl Space {
    /// The status of this space: what the member's copy holds that a read
    /// does not show.
    pub fn status(&self) -> SpaceStatusReport<'_> {
        SpaceStatusReport { space: self }
    }
}

impl SpaceStatusReport<'_> {
    /// The status as it is now.
    pub fn current(&self) -> SpaceStatus {
        status(&crate::db::lock(self.space.inner()), self.space.id().0)
    }

    /// Follows the status: the stream yields it at once, and again
    /// whenever it changes. Dropping the stream stops it.
    pub fn subscribe(&self) -> impl Stream<Item = SpaceStatus> + Unpin + use<> {
        let space = self.space.id().0;
        crate::sync::follow(self.space.inner(), move |inner: &Inner| {
            status(inner, space)
        })
    }
}

/// The status of `space` in the database `inner`.
fn status(inner: &Inner, space: CoreSpaceId) -> SpaceStatus {
    let me = inner.member.key().id();
    let (initial_sync_complete, behind) = crate::sync::progress(inner, space);
    let counts = inner.view.state_counts(space);
    let mut parked = BTreeMap::new();
    let changes = counts.parked.iter().map(|(r, n)| (reasons::park(*r), n));
    let operations = counts
        .parked_operations
        .iter()
        .map(|(r, n)| (reasons::park_operation(*r), n));
    for (reason, count) in changes.chain(operations) {
        if let Some(reason) = reason {
            *parked.entry(reason).or_default() += count;
        }
    }
    let mut rejected = BTreeMap::new();
    for (reason, count) in &counts.rejected {
        if let Some(reason) = reasons::reject(*reason) {
            *rejected.entry(reason).or_default() += count;
        }
    }
    let refused = inner
        .view
        .log(space)
        .map(|log| {
            log.refused_by(me)
                .map(|(change, refusal)| Refused {
                    change: ChangeId(change),
                    code: crate::sync::mapped_code(&refusal.0),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut status = SpaceStatus {
        role: None,
        initial_sync_complete,
        behind,
        gaps: Vec::new(),
        parked,
        newest_model: None,
        rejected,
        refused,
        lost_grants: Vec::new(),
        equivocations: Vec::new(),
        exposed: false,
        waiting_for_key: false,
        bad_person_key_records: Vec::new(),
        provisional_erasures: Vec::new(),
    };
    let Some(state) = inner.spaces.get(&space) else {
        return status;
    };
    let resolved = state.checks.resolved();
    if !crate::sync::stopped(inner, space) {
        status.role = resolved.role(me).map(Role::of);
    }
    status.gaps = resolved.gaps().iter().map(|id| ChangeId(*id)).collect();
    status.lost_grants = resolved
        .verdicts()
        .iter()
        .filter(|(_, verdict)| **verdict == Verdict::Lost)
        .map(|(id, _)| ChangeId(*id))
        .collect();
    if let Some(log) = inner.view.log(space) {
        status.equivocations = log.equivocations().into_iter().map(ChangeId).collect();
    }
    status.bad_person_key_records = state
        .bad_person_key_records
        .iter()
        .map(|account| crate::erasures::of_account_key(resolved, *account))
        .collect();
    let now = crate::clock::now_millis();
    status.provisional_erasures = pending(resolved)
        .map(|(shred, performer)| {
            let held_at = inner.view.shred_held_at(space, shred).ok().flatten();
            ProvisionalErasure {
                change: ChangeId(shred),
                age: Duration::from_millis(now.saturating_sub(held_at.unwrap_or(now))),
                can_undo: resolved.role(me) == Some(CoreRole::Admin)
                    && resolved.same_account(me, performer),
                performer: MemberId(performer),
                notice: Notice::ERASURE_PENDING,
            }
        })
        .collect();
    status.newest_model = inner
        .view
        .newest_model(space, &state.checks)
        .map(|version| u32::try_from(version).unwrap_or(u32::MAX));
    if let Some(current) = resolved.current_generation() {
        status.exposed = resolved.exposed(current);
        status.waiting_for_key = status.role.is_some() && state.checks.space_key(current).is_none();
    }
    status
}

/// The provisional shreds of a space that no accepted decision ended yet,
/// with their performers, in change id order.
pub(crate) fn pending(
    resolved: &Resolved,
) -> impl Iterator<Item = (ncdb_core::ids::ChangeId, ncdb_core::ids::MemberId)> + '_ {
    let mut pending: Vec<_> = resolved
        .person_keys()
        .filter(|(_, key)| key.provisional && key.finalized.is_none() && key.taken_back.is_none())
        .filter_map(|(_, key)| Some((key.shred?, key.performer()?)))
        .collect();
    pending.sort();
    pending.into_iter()
}

/// Notes in the view, with the current time, every provisional shred of
/// `space` that this device holds, so that its age, and the 7 days after
/// which a device of the account finalizes it, count from when this device
/// first held it; a shred noted before keeps its time (NOC-218 A).
pub(crate) fn note_shreds(
    view: &mut View,
    space: CoreSpaceId,
    resolved: &Resolved,
) -> Result<(), ViewError> {
    let now = crate::clock::now_millis();
    for (shred, _) in pending(resolved) {
        view.note_shred_held(space, shred, now)?;
    }
    Ok(())
}
