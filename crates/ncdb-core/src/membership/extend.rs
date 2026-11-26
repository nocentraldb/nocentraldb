//! The fold of a set plus one change (NOC-442). Step 1 judges each node
//! from the fold of its causal past (RFC 0008, "The function"), and folding
//! each past from scratch costs the square of the control changes: about
//! 50 million nodes walked to open a space with 10,000 persons.
//!
//! A [`Running`] fold is steps 2 to 5 over a set closed under dependencies,
//! with what step 5's walk leaves behind. A person-key entry whose causal
//! past is exactly that set comes after every node of it in the order of
//! step 4, and changes neither seniority (step 2) nor any revocation edge
//! between the nodes of the set (step 3): it is not a revocation, and
//! nothing depends on it. So the fold of the set with it is the fold of
//! the set, with the entry judged as step 5 judges the last change of the
//! order. Every other change is folded from scratch, as before.
//!
//! `resolve` stays the definition: the convergence tests check a
//! [`super::Resolver`], which folds this way, against it in every order of
//! arrival.

use std::collections::BTreeSet;

use crate::ids::ChangeId;

use super::body::Entry;
use super::input::{Input, Node};
use super::judge::Judged;
use super::past::Past;
use super::resolve::{Resolved, Walk, fold};
use super::rights;
use super::state::{Rejection, Verdict};

/// Steps 2 to 5 over a set of nodes closed under dependencies, none with a
/// gap in its causal past.
pub(super) struct Running {
    set: Past,
    folded: Resolved,
    walk: Walk,
}

impl Running {
    /// The fold of `set` from scratch.
    pub(super) fn new(input: &Input, judged: &Judged, set: &Past) -> Running {
        let ids: BTreeSet<ChangeId> = input.nodes_of(set).map(|node| node.id).collect();
        let (folded, walk) = fold(input, judged, &ids);
        Running {
            set: set.clone(),
            folded,
            walk,
        }
    }

    /// Whether this is the fold of exactly `set`.
    pub(super) fn covers(&self, set: &Past) -> bool {
        self.set == *set
    }

    /// The state of the set.
    pub(super) fn folded(&self) -> &Resolved {
        &self.folded
    }

    /// Whether [`Running::extend`] adds `node`: it is a person-key entry
    /// without a gap, its causal past is exactly the set, the set has a
    /// genesis, and its author already has a place in the seniority.
    pub(super) fn admits(&self, node: &Node) -> bool {
        matches!(node.body.entry, Entry::PersonKey(_))
            && !node.gap
            && *node.past == self.set
            && self.folded.genesis.is_some()
            && self.folded.seniority.contains(&node.author)
    }

    /// Adds `node`, which step 1 has judged, when the fold of the set with
    /// it is this fold plus the node: it is a person-key entry without a
    /// gap, its causal past is exactly the set, the set has a genesis, and
    /// its author already has a place in the seniority. Returns whether it
    /// did; otherwise nothing changed.
    pub(super) fn extend(&mut self, input: &Input, judged: &Judged, node: &Node) -> bool {
        let Entry::PersonKey(entry) = &node.body.entry else {
            return false;
        };
        if !self.admits(node) {
            return false;
        }
        let state = &mut self.folded;
        let walk = &mut self.walk;
        // Step 5 for the last change of the order, as `apply_order` judges
        // it; a person-key entry is none that is final when valid, nor an
        // add that a statement entry supersedes.
        let excluded = walk
            .revocations
            .iter()
            .find(|r| rights::excludes(&input.nodes[r], node, input, judged));
        let mut verdict = if !judged.valid(node.id) {
            Verdict::Rejected(Rejection::NotValidWhenWritten)
        } else if let Some(by) = excluded {
            Verdict::Rejected(Rejection::Excluded(*by))
        } else if state.members.role(node.author) < Some(rights::needed(node)) {
            Verdict::Rejected(Rejection::NoRight)
        } else if !walk.keys.admits(node) {
            Verdict::Rejected(Rejection::Commitment)
        } else {
            let past_members = judged.past_members(node.id);
            let provisional = judged.provisional(node.id);
            walk.keys
                .apply(node, provisional, &past_members, &BTreeSet::new());
            Verdict::Accepted
        };
        // The rules for the whole set: no accepted shred of its id saw it.
        if let Some(shred) = walk.shreds.get(&entry.person_key)
            && verdict == Verdict::Accepted
        {
            verdict = Verdict::Rejected(Rejection::Shredded(*shred));
        }
        if verdict == Verdict::Accepted {
            let past_members = judged.past_members(node.id);
            let provisional = judged.provisional(node.id);
            state
                .person_keys
                .apply(node, provisional, &past_members, &walk.taken_back);
        }
        state.verdicts.insert(node.id, verdict);
        state.order.push(node.id);
        self.set.insert(node.number);
        true
    }
}
