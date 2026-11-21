//! `resolve` with a memory (NOC-424). A member resolves the control
//! changes of a space again after each one it adds, and the SDK writes a
//! control change for every new person, so a space with many persons holds
//! many of them. Step 1 judges each node from the fold of its causal past
//! (RFC 0008, "The function"), and a fresh call folds every past again:
//! the work grows with the square of the control changes, on every call.
//!
//! A [`Resolver`] remembers what depends on one change alone (its body,
//! whether its signature verifies) and on one node's causal past alone (the
//! past itself, and the node's judgement), and takes it from there while
//! it still holds. Every call still settles the whole set (steps 2 to 5),
//! so the state it returns is `resolve`'s, byte for byte: a new control
//! change after a gap-free past costs the judgement of that change and one
//! pass over the nodes, instead of a fold of every node's past.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::change::Change;
use crate::ids::{ChangeId, MemberId, SpaceId};

use super::body::Entry;
use super::input::{self, Input};
use super::judge::{Judged, Judgement, judge};
use super::resolve::{Resolved, fold, settle};

/// RFC 0008's `resolve` for one space, with a memory of its earlier calls.
/// Each call takes the whole set of control changes, as [`super::resolve`]
/// does, and returns what it returns; the sets need not grow from one call
/// to the next.
pub struct Resolver {
    space: SpaceId,
    inputs: input::Memo,
    /// The judgement of each node judged without a gap in its causal past,
    /// with that past: a function of the past alone (RFC 0008, step 1).
    judgements: BTreeMap<ChangeId, Remembered>,
    judged: usize,
}

/// A node's judgement, the past it was judged from, and for a valid shred
/// the members of that past's state.
struct Remembered {
    past: Arc<std::collections::BTreeSet<ChangeId>>,
    judgement: Judgement,
    shred_past_members: Option<std::collections::BTreeSet<MemberId>>,
}

impl Resolver {
    /// A resolver of `space` that remembers nothing yet.
    pub fn new(space: SpaceId) -> Resolver {
        Resolver {
            space,
            inputs: input::Memo::default(),
            judgements: BTreeMap::new(),
            judged: 0,
        }
    }

    /// The control state of the control changes of the space among
    /// `changes`: [`super::resolve`]'s.
    pub fn resolve<'a>(&mut self, changes: impl IntoIterator<Item = &'a Change>) -> Resolved {
        let input = Input::new(self.space, changes, &mut self.inputs);
        let mut judged = Judged::default();
        self.judged = 0;
        // Smaller causal pasts first: a node's past holds only nodes with
        // smaller pasts, so every fold below finds their judgement.
        let mut nodes: Vec<&input::Node> = input.nodes.values().collect();
        nodes.sort_by_key(|node| (node.past.len(), node.id));
        for node in nodes {
            // A judgement is remembered for the very same past, without a
            // gap: the nodes of the past, their own pasts and judgements
            // are then those it was judged from.
            let same = self
                .judgements
                .get(&node.id)
                .filter(|before| !node.gap && Arc::ptr_eq(&before.past, &node.past));
            if let Some(before) = same {
                if let Some(members) = &before.shred_past_members {
                    judged.shred_past_members.insert(node.id, members.clone());
                }
                judged.judgements.insert(node.id, before.judgement);
                continue;
            }
            self.judged += 1;
            let past = fold(&input, &judged, &node.past);
            let judgement = judge(&input, &judged, node, &past);
            let mut shred_past_members = None;
            if judgement.valid && matches!(node.body.entry, Entry::Shred(_)) {
                let members: std::collections::BTreeSet<MemberId> =
                    past.members().map(|(member, _)| member).collect();
                judged.shred_past_members.insert(node.id, members.clone());
                shred_past_members = Some(members);
            }
            judged.judgements.insert(node.id, judgement);
            if !node.gap {
                let remembered = Remembered {
                    past: Arc::clone(&node.past),
                    judgement,
                    shred_past_members,
                };
                self.judgements.insert(node.id, remembered);
            }
        }
        settle(input, &judged)
    }

    /// How many control changes the last call judged from the fold of
    /// their causal past, rather than taking the judgement from its
    /// memory.
    pub fn judged(&self) -> usize {
        self.judged
    }
}
