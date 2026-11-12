//! Step 1 of the function (RFC 0008, "The function"): whether a node is
//! valid when written, judged from the fold of its causal past alone, with
//! what else that fold settles for it: whether a shred is provisional, the
//! members of a shred's causal past, and the depth of a grant of admin.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId};

use super::body::{Entry, Role};
use super::input::{Input, Node};
use super::resolve::Resolved;
use super::rights;

/// What step 1 found for a node, which depends only on its causal past.
#[derive(Clone, Copy)]
pub(crate) struct Judgement {
    pub(crate) valid: bool,
    /// For a valid shred: whether the performer's own account signed it
    /// off, so that it is provisional (RFC 0008, "Shred entry").
    pub(crate) provisional: bool,
    /// For a valid grant of admin: its depth (step 2).
    pub(crate) depth: Option<u64>,
}

/// The judgement of every node judged so far, and for each valid shred
/// the members of the state of its causal past.
#[derive(Default)]
pub(crate) struct Judged(
    pub(crate) BTreeMap<ChangeId, Judgement>,
    pub(crate) BTreeMap<ChangeId, BTreeSet<MemberId>>,
);

impl Judged {
    pub(crate) fn valid(&self, id: ChangeId) -> bool {
        self.0.get(&id).is_some_and(|j| j.valid)
    }

    pub(crate) fn provisional(&self, id: ChangeId) -> bool {
        self.0.get(&id).is_some_and(|j| j.provisional)
    }

    /// The members of the state of a valid shred's causal past.
    pub(crate) fn past_members(&self, id: ChangeId) -> BTreeSet<MemberId> {
        self.1.get(&id).cloned().unwrap_or_default()
    }
}

/// Step 1 for one node, given the fold of its causal past.
pub(crate) fn judge(input: &Input, judged: &Judged, node: &Node, past: &Resolved) -> Judgement {
    if node.is_genesis() {
        return Judgement {
            valid: true,
            provisional: false,
            depth: Some(0),
        };
    }
    let valid = past.genesis().is_some()
        && past.role(node.author) >= Some(rights::needed(node))
        && rights::own_rules(input.space, node, past);
    let depth = match (valid, node.membership()) {
        (true, Some(entry)) if entry.role() == Some(Role::Admin) => {
            let held = past.member(node.author).into_iter().flat_map(|s| &s.grants);
            held.filter(|g| g.effective == Some(Role::Admin))
                .filter_map(|g| match input.nodes.get(&g.change) {
                    Some(n) if n.is_genesis() => Some(0),
                    _ => judged.0.get(&g.change).and_then(|j| j.depth),
                })
                .min()
                .map(|depth| depth + 1)
        }
        _ => None,
    };
    let provisional = valid
        && match &node.body.entry {
            Entry::Shred(shred) => {
                rights::signed_off(input.space, node, shred, past)
                    == Some(rights::SignedOff::Provisional)
            }
            _ => false,
        };
    Judgement {
        valid,
        provisional,
        depth,
    }
}
