//! `resolve` with a memory (NOC-424, NOC-442). A member resolves the
//! control changes of a space again after each one it adds, and the SDK
//! writes a control change for every new person, so a space with many
//! persons holds many of them. Step 1 judges each node from the fold of its
//! causal past (RFC 0008, "The function"), and folding every past from
//! scratch grows with the square of the control changes.
//!
//! A [`Resolver`] remembers what depends on one change alone (its body,
//! whether its signature verifies) and on one node's causal past alone (the
//! past itself, and the node's judgement), and takes it from there while
//! it still holds. It also carries one fold of a set without a gap on from
//! node to node and from call to call: a person-key entry whose past is
//! exactly that set adds itself to that fold ([`Running`]), and every other
//! node is folded from scratch. Opening a space whose control changes are
//! a chain of person-key entries walks each of them a bounded number of
//! times, and a new person-key entry after them costs its own judgement.
//! The state a call returns is `resolve`'s, byte for byte, which the
//! convergence tests check against [`Resolver::from_scratch`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::change::Change;
use crate::ids::{ChangeId, MemberId, SpaceId};

use super::body::Entry;
use super::extend::Running;
use super::input::{self, Input};
use super::judge::{Judged, Judgement, judge};
use super::past::Past;
use super::resolve::{Resolved, finish};

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
    /// The last fold of a set without a gap: the fold of the past of a node
    /// whose past is exactly that set, as a change written after all of
    /// them has, and the start of the fold of the set with that node
    /// (NOC-442).
    running: Option<Running>,
    /// Whether a fold may be carried on from the fold of the set before;
    /// without, every node's past is folded from scratch.
    extends: bool,
    judged: usize,
    folded: usize,
}

/// A node's judgement, the past it was judged from, and for a valid shred
/// the members of that past's state.
struct Remembered {
    past: Arc<Past>,
    judgement: Judgement,
    shred_past_members: Option<BTreeSet<MemberId>>,
}

impl Resolver {
    /// A resolver of `space` that remembers nothing yet.
    pub fn new(space: SpaceId) -> Resolver {
        Resolver {
            space,
            inputs: input::Memo::default(),
            judgements: BTreeMap::new(),
            running: None,
            extends: true,
            judged: 0,
            folded: 0,
        }
    }

    /// A resolver of `space` that folds the causal past of every node it
    /// judges from scratch, and the whole set too, as RFC 0008 defines
    /// `resolve` ("The function"): the oracle the tests check a
    /// [`Resolver`] against (NOC-442). Its cost grows with the square of
    /// the control changes.
    #[doc(hidden)]
    pub fn from_scratch(space: SpaceId) -> Resolver {
        Resolver {
            extends: false,
            ..Resolver::new(space)
        }
    }

    /// The control state of the control changes of the space among
    /// `changes`: [`super::resolve`]'s.
    pub fn resolve<'a>(&mut self, changes: impl IntoIterator<Item = &'a Change>) -> Resolved {
        let input = Input::new(self.space, changes, &mut self.inputs);
        let mut judged = Judged::default();
        self.judged = 0;
        self.folded = 0;
        let mut running = self.running.take().filter(|_| self.extends);
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
            let mut fresh = None;
            if let Some(before) = same {
                if let Some(members) = &before.shred_past_members {
                    judged.shred_past_members.insert(node.id, members.clone());
                }
                judged.judgements.insert(node.id, before.judgement);
            } else {
                self.judged += 1;
                let covered = running.as_ref().filter(|r| !node.gap && r.covers(&node.past));
                let past = match covered {
                    Some(running) => running.folded(),
                    None => {
                        self.folded += node.past.len();
                        fresh.insert(Running::new(&input, &judged, &node.past)).folded()
                    }
                };
                let judgement = judge(&input, &judged, node, past);
                let mut shred_past_members = None;
                if judgement.valid && matches!(node.body.entry, Entry::Shred(_)) {
                    let members: BTreeSet<MemberId> =
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
            if let Some(fresh) = fresh.filter(|_| !node.gap && self.extends) {
                running = Some(fresh);
            }
            if let Some(running) = &mut running {
                running.extend(&input, &judged, node);
            }
        }
        let all: Past = input.nodes.values().map(|node| node.number).collect();
        let gap_free = input.nodes.values().all(|node| !node.gap);
        let folded = match running {
            Some(running) if gap_free && running.covers(&all) => {
                let folded = running.folded().clone();
                self.running = Some(running);
                folded
            }
            running => {
                self.folded += input.nodes.len();
                let all = Running::new(&input, &judged, &all);
                let folded = all.folded().clone();
                self.running = if gap_free { Some(all) } else { running };
                folded
            }
        };
        finish(input, folded)
    }
    /// How many control changes the last call judged from the fold of
    /// their causal past, rather than taking the judgement from its
    /// memory.
    pub fn judged(&self) -> usize {
        self.judged
    }

    /// How many nodes the last call walked in folds from scratch (steps 2
    /// to 5), to judge a control change or to settle the set, rather than
    /// adding a change to the fold of its past (NOC-442).
    pub fn folded(&self) -> usize {
        self.folded
    }
}
