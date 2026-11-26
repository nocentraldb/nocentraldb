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
//!
//! [`Resolver::resolve`] still reads every change of the set it is given,
//! so its cost grows with the control changes even when it judges one. A
//! holder that keeps the resolver while its set only grows, such as the
//! relay, adds a new person-key entry with [`Resolver::add`] instead,
//! which reads only that entry (NOC-497).

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
    /// The input of the last call and its judgements, while the running
    /// fold is the fold of all its nodes: what [`Resolver::add`] adds to
    /// (NOC-497).
    last: Option<Last>,
    read: usize,
    judged: usize,
    folded: usize,
}

/// The input of the last call, and the judgement of each of its nodes.
struct Last {
    input: Input,
    judged: Judged,
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
            last: None,
            read: 0,
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
        self.last = None;
        let input = Input::new(self.space, changes, &mut self.inputs);
        let mut judged = Judged::default();
        self.read = input.len();
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
                let covered = running
                    .as_ref()
                    .filter(|r| !node.gap && r.covers(&node.past));
                let past = match covered {
                    Some(running) => running.folded(),
                    None => {
                        self.folded += node.past.len();
                        fresh
                            .insert(Running::new(&input, &judged, &node.past))
                            .folded()
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
        let resolved = finish(&input, folded);
        // The running fold is now the fold of every node exactly when the
        // set has no gap.
        if gap_free && self.extends {
            self.last = Some(Last { input, judged });
        }
        resolved
    }

    /// The control state of the set of the last call with `changes` added
    /// in this order: what [`Resolver::resolve`] returns for that set
    /// (NOC-497). It adds a change without reading the set again when the
    /// change is a person-key entry of the space whose causal past is
    /// exactly that set, from an author the set made a member, and which
    /// no change of the set depends on: as [`Resolver::resolve`] does, it
    /// judges it and adds it to the fold of the set. None for any other
    /// change, or when the last call's set has a gap; the caller then
    /// resolves the whole set, and until it does, every call to this gives
    /// None.
    pub fn add<'a>(&mut self, changes: impl IntoIterator<Item = &'a Change>) -> Option<Resolved> {
        self.read = 0;
        self.judged = 0;
        self.folded = 0;
        let mut last = self.last.take()?;
        for change in changes {
            self.read += 1;
            if !self.add_one(&mut last, change) {
                return None;
            }
        }
        let folded = self.running.as_ref()?.folded().clone();
        let resolved = finish(&last.input, folded);
        self.last = Some(last);
        Some(resolved)
    }

    /// Adds one change to `last` and the running fold, which is the fold
    /// of all its nodes; false, and the running fold unchanged, when
    /// [`Resolver::add`] cannot add it.
    fn add_one(&mut self, last: &mut Last, change: &Change) -> bool {
        let Some(running) = self.running.as_mut() else {
            return false;
        };
        let Some(node) = last.input.person_key_node(change, &mut self.inputs) else {
            return false;
        };
        if !running.admits(&node) {
            return false;
        }
        // Step 1, as `resolve` judges it: from the memory while the past
        // is the very same, else from the fold of the past, which is the
        // running fold.
        let same = self
            .judgements
            .get(&node.id)
            .filter(|before| Arc::ptr_eq(&before.past, &node.past))
            .map(|before| before.judgement);
        let judgement = same.unwrap_or_else(|| {
            self.judged += 1;
            let judgement = judge(&last.input, &last.judged, &node, running.folded());
            let remembered = Remembered {
                past: Arc::clone(&node.past),
                judgement,
                shred_past_members: None,
            };
            self.judgements.insert(node.id, remembered);
            judgement
        });
        let id = node.id;
        last.judged.judgements.insert(id, judgement);
        last.input.insert(node);
        running.extend(&last.input, &last.judged, &last.input.nodes[&id])
    }

    /// How many control changes of the space the last call read: every
    /// one of the set for [`Resolver::resolve`], the added ones for
    /// [`Resolver::add`].
    pub fn read(&self) -> usize {
        self.read
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
