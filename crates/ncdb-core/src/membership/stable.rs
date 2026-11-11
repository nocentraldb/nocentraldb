//! When a control change is stable (RFC 0008, "Effects that wait until a
//! change is stable", decisions M20 and M21): once nothing can reject it any
//! more, so that a shred may destroy its person key for good.
//!
//! A holder's stable changes are the smallest set closed under RFC 0008's
//! rule. An accepted control change c is stable when, for every member that
//! holds the admin role, or held it and lost it to an accepted revocation,
//! the holder holds that member's changes from author sequence 1 up to one
//! of: a control change of that member whose causal past contains c; or the
//! cutoff of a stable revocation that took the role from that member. The
//! holder's runs come from the caller, because the rule counts every change
//! of the member, sealed ones too, and the fold sees control changes only.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId};

use super::body::{Entry, Role};
use super::input::Input;
use super::state::{Members, Verdict};

/// Which control changes are stable for one holder, and what each waiting
/// shred waits for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stability {
    stable: BTreeSet<ChangeId>,
    waiting: BTreeMap<ChangeId, BTreeSet<MemberId>>,
}

impl Stability {
    /// Whether this accepted control change is stable.
    pub fn is_stable(&self, change: ChangeId) -> bool {
        self.stable.contains(&change)
    }

    /// The members whose runs block this accepted shred entry, or none when
    /// it is stable or is no accepted shred.
    pub fn blocking(&self, shred: ChangeId) -> Option<&BTreeSet<MemberId>> {
        self.waiting.get(&shred)
    }

    /// The accepted shred entries that are not stable yet, each with the
    /// members whose runs block it.
    pub fn shreds_waiting(&self) -> impl Iterator<Item = (ChangeId, &BTreeSet<MemberId>)> + '_ {
        self.waiting
            .iter()
            .map(|(shred, blocking)| (*shred, blocking))
    }
}

/// One member the rule counts, with what may bound its run.
struct Counted {
    member: MemberId,
    /// The sequences and causal pasts of its control changes.
    changes: Vec<(u64, BTreeSet<ChangeId>)>,
    /// The accepted revocations that took the admin role from it, with their
    /// cutoffs; none for a member that holds the role now.
    revocations: Vec<(ChangeId, u64)>,
}

/// The stability of every accepted control change of `input`, under the
/// membership `members` and the verdicts the fold gave; `held(m)` is the
/// highest author sequence n such that the holder holds m's changes 1 to n.
pub(crate) fn compute(
    input: &Input,
    members: &Members,
    verdicts: &BTreeMap<ChangeId, Verdict>,
    held: impl Fn(MemberId) -> u64,
) -> Stability {
    let accepted =
        |id: &ChangeId| matches!(verdicts.get(id), Some(Verdict::Accepted | Verdict::Lost));
    let counted: Vec<Counted> = members
        .0
        .iter()
        .filter(|(_, state)| {
            state.role == Some(Role::Admin) || state.grants.iter().any(|g| g.role == Role::Admin)
        })
        .map(|(member, state)| Counted {
            member: *member,
            changes: input
                .nodes
                .values()
                .filter(|node| node.author == *member)
                .map(|node| (node.sequence, node.past.clone()))
                .collect(),
            revocations: match state.role {
                Some(Role::Admin) => Vec::new(),
                _ => state
                    .revocations
                    .iter()
                    .filter(|r| r.role < Some(Role::Admin) && accepted(&r.change))
                    .map(|r| (r.change, r.cutoff))
                    .collect(),
            },
        })
        .collect();
    let candidates: Vec<ChangeId> = input.nodes.keys().copied().filter(accepted).collect();

    // A member's run reaches c when the holder holds it up to the first of
    // its control changes that saw c, or up to the cutoff of a stable
    // revocation that took its role.
    let reaches = |m: &Counted, c: ChangeId, stable: &BTreeSet<ChangeId>| {
        let saw = m
            .changes
            .iter()
            .filter(|(_, past)| past.contains(&c))
            .map(|(sequence, _)| *sequence);
        let bounded = m
            .revocations
            .iter()
            .filter(|(r, _)| stable.contains(r))
            .map(|(_, cutoff)| *cutoff);
        saw.chain(bounded)
            .min()
            .is_some_and(|bound| held(m.member) >= bound)
    };

    let mut stable = BTreeSet::new();
    loop {
        let added: Vec<ChangeId> = candidates
            .iter()
            .copied()
            .filter(|c| !stable.contains(c))
            .filter(|c| counted.iter().all(|m| reaches(m, *c, &stable)))
            .collect();
        if added.is_empty() {
            break;
        }
        stable.extend(added);
    }

    let waiting = candidates
        .iter()
        .copied()
        .filter(|c| !stable.contains(c))
        .filter(|c| matches!(input.nodes[c].body.entry, Entry::Shred(_)))
        .map(|c| {
            let blocking = counted
                .iter()
                .filter(|m| !reaches(m, c, &stable))
                .map(|m| m.member)
                .collect();
            (c, blocking)
        })
        .collect();
    Stability { stable, waiting }
}
