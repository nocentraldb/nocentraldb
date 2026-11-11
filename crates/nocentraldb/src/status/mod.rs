//! The status of one space (RFC 0009, "Sync and status": `space.status`):
//! everything a space holds that an app does not see in a read, with the
//! Rust names the owner decided on NOC-193. `space.status()` gives a
//! [`SpaceStatusReport`], whose `current()` gives a [`SpaceStatus`] and
//! whose `subscribe()` follows it.
//!
//! **What this does not report yet.** RFC 0009 lists more than this
//! status holds. `newest_model`, the highest model version among the
//! parked changes and operations, and the reason "another model" need the
//! writer's model in each change (RFC 0002, payload key 64), which the
//! core does not read yet. The struct is `#[non_exhaustive]`, so each
//! field comes when it is built, instead of a field that always reads
//! empty and claims nothing waits.
//!
//! **A shred's age.** A control change carries no clock (RFC 0008, "The
//! control bodies"), so a shred's age counts from when this device first
//! held it, which the view keeps ([`note_shreds`]; NOC-218). A device that
//! joins later, or a fresh install, starts the count again: the alarm
//! comes late, never early.

mod reasons;

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_core::Stream;
use ncdb_core::checks::SealChecks;
use ncdb_core::ids::{ChangeId as CoreChangeId, MemberId, SpaceId as CoreSpaceId};
use ncdb_core::membership::{Resolved, Verdict};
use ncdb_core::view::{Error as ViewError, View};

use crate::db::Inner;
use crate::id::ChangeId;
use crate::members::{Fingerprint, Role};
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
    /// The parked changes and operations, per reason.
    pub parked: BTreeMap<ParkReason, u64>,
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
    /// The accepted shreds that are not stable yet, in change id order: a
    /// shred destroys its person key for good only once nothing can reject
    /// it any more (RFC 0008, "Effects that wait until a change is
    /// stable").
    pub shreds_waiting: Vec<ShredWaiting>,
}

/// A shred that is not stable yet.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShredWaiting {
    /// The shred entry.
    pub change: ChangeId,
    /// How long ago this device first held it.
    pub age: Duration,
    /// The accounts of the admin devices whose runs block it, each once,
    /// in the order of their account keys. Filled only on an admin device
    /// once the shred waited 7 days; empty before, and on any other device.
    pub blocking: Vec<Fingerprint>,
}

/// How long a shred waits before an admin device names the accounts that
/// block it (RFC 0008, "Effects that wait until a change is stable": 7
/// days).
const ALARM: Duration = Duration::from_secs(7 * 24 * 60 * 60);

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
        rejected,
        refused,
        lost_grants: Vec::new(),
        equivocations: Vec::new(),
        exposed: false,
        waiting_for_key: false,
        shreds_waiting: Vec::new(),
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
    if let Some(current) = resolved.current_generation() {
        status.exposed = resolved.exposed(current);
        status.waiting_for_key = status.role.is_some() && state.checks.space_key(current).is_none();
    }
    let admin = status.role == Some(Role::Admin);
    status.shreds_waiting = shreds_waiting(&inner.view, space, &state.checks, admin);
    status
}

/// The shreds of `space` that wait, as [`SpaceStatus::shreds_waiting`]
/// gives them; `admin` says whether this device is an admin of the space.
fn shreds_waiting(
    view: &View,
    space: CoreSpaceId,
    checks: &SealChecks,
    admin: bool,
) -> Vec<ShredWaiting> {
    let resolved = checks.resolved();
    if shreds(resolved).next().is_none() {
        return Vec::new();
    }
    let runs = view.log(space).map(|log| log.runs()).unwrap_or_default();
    let stability = checks.stability(|member| runs.get(&member).copied().unwrap_or(0));
    let now = now_millis();
    stability
        .shreds_waiting()
        .map(|(shred, members)| {
            let held = view
                .shred_held_at(space, shred)
                .ok()
                .flatten()
                .unwrap_or(now);
            let age = Duration::from_millis(now.saturating_sub(held));
            let blocking = if admin && age >= ALARM {
                accounts(resolved, members)
            } else {
                Vec::new()
            };
            ShredWaiting {
                change: ChangeId(shred),
                age,
                blocking,
            }
        })
        .collect()
}

/// The accounts of `members`, each once, in the order of their account
/// keys.
fn accounts(resolved: &Resolved, members: &BTreeSet<MemberId>) -> Vec<Fingerprint> {
    let keys: BTreeSet<[u8; 32]> = members
        .iter()
        .filter_map(|member| resolved.member(*member))
        .map(|state| state.certificate.account_key)
        .collect();
    keys.into_iter().map(Fingerprint).collect()
}

/// The accepted shreds of a space.
fn shreds(resolved: &Resolved) -> impl Iterator<Item = CoreChangeId> + '_ {
    resolved.person_keys().filter_map(|(_, key)| key.shred)
}

/// Records in `view` that this device holds every accepted shred the
/// checks know, now, unless it held it before: the start of a shred's age.
/// The SDK calls it after it folds changes and when it opens a space.
pub(crate) fn note_shreds(view: &mut View, checks: &SealChecks) -> Result<(), ViewError> {
    let resolved = checks.resolved();
    let mut held = shreds(resolved).peekable();
    if held.peek().is_none() {
        return Ok(());
    }
    let now = now_millis();
    let space = checks.space();
    for shred in held {
        view.note_shred_held(space, shred, now)?;
    }
    Ok(())
}

/// Now, in milliseconds since the Unix epoch.
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}
