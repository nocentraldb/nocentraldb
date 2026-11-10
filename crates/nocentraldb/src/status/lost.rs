//! Lost grants (RFC 0008, "Concurrent add and remove of one member"): "a
//! grant that did not see the removal", which the removal "caps … at
//! 'none'", so that the grant "stays accepted but without effect, and the
//! SDK shows it as a lost grant". The grant and the revocation that caps
//! it are concurrent: neither saw the other, and the grant's author can
//! repeat it once it has seen the revocation.
//!
//! The core's `Verdict::Lost` marks every grant whose role a revocation it
//! did not see lowered, which takes in a grant that a later revocation
//! revoked after it saw the grant: a revoked grant, not a lost one. So the
//! SDK keeps only a `Lost` grant that some revocation of its subject caps
//! while neither saw the other (NOC-190, pm's call A), reading the
//! resolved membership and the control dependencies in the log, and
//! changing neither. A later core change may make the verdict match RFC
//! 0008, and this filter go.

use std::collections::BTreeSet;

use ncdb_core::change::Kind;
use ncdb_core::ids::ChangeId;
use ncdb_core::log::Log;
use ncdb_core::membership::{ControlBody, Entry, Resolved, Verdict};

/// The control body of the change `id`, when the log holds it as a
/// control change.
fn body(log: &Log, id: ChangeId) -> Option<ControlBody> {
    let change = log.get(log.position_of(id)?)?;
    if change.header().kind != Kind::CONTROL {
        return None;
    }
    ControlBody::decode(&change.header().payload).ok()
}

/// Whether `target` is in the causal past of the control change `from`,
/// through the control dependencies the log holds.
fn saw(log: &Log, from: ChangeId, target: ChangeId) -> bool {
    let mut seen = BTreeSet::new();
    let mut next = vec![from];
    while let Some(id) = next.pop() {
        let Some(body) = body(log, id) else { continue };
        for dependency in body.dependencies {
            if dependency == target {
                return true;
            }
            if seen.insert(dependency) {
                next.push(dependency);
            }
        }
    }
    false
}

/// The lost grants of a space, in change id order: each accepted grant
/// that a concurrent revocation of its subject capped below the role it
/// gives.
pub(super) fn lost_grants(resolved: &Resolved, log: &Log) -> Vec<ChangeId> {
    resolved
        .verdicts()
        .iter()
        .filter(|(_, verdict)| **verdict == Verdict::Lost)
        .map(|(id, _)| *id)
        .filter(|grant| concurrently_capped(resolved, log, *grant))
        .collect()
}

/// Whether a revocation of the grant's subject caps the grant below its
/// role while neither saw the other.
fn concurrently_capped(resolved: &Resolved, log: &Log, grant: ChangeId) -> bool {
    let Some(Entry::Membership(entry)) = body(log, grant).map(|body| body.entry) else {
        return false;
    };
    let Some(subject) = resolved.member(entry.subject) else {
        return false;
    };
    let Some(given) = subject.grants.iter().find(|g| g.change == grant) else {
        return false;
    };
    let seen_by_grant = resolved.seen().get(&grant);
    subject.revocations.iter().any(|revocation| {
        revocation.role < Some(given.role)
            && !seen_by_grant.is_some_and(|seen| seen.contains(&revocation.change))
            && !saw(log, revocation.change, grant)
    })
}
