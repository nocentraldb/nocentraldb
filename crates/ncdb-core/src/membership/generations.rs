//! The generation graph of RFC 0008 ("Generation ids"): the space's first
//! generation, and every generation a key chain entry that is valid when
//! written names, accepted or later rejected (decision M17), each linked to
//! its previous generations. "Descends from" follows those links.
//!
//! Each generation also records who may hold it ("Two removals that rotate
//! the key at once", step 4): the author of its key chain entry, the
//! creator for the first generation, every recipient of an envelope for
//! it, and everyone who may hold a generation that descends from it, since
//! the key chain of that generation opens it.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId};

use super::body::Entry;
use super::input::Node;
use super::resolve::Judged;

/// One generation of the space key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Generation {
    /// The generations it follows; none for the first.
    pub previous: BTreeSet<u64>,
    /// The key chain entries that name it, valid when written, with their
    /// authors; none for the first. Two of them share an id only through
    /// equivocation or a hash collision (decision M23).
    pub entries: BTreeMap<ChangeId, MemberId>,
    /// The recipients of every key envelope for it in the set, whatever its
    /// verdict, since a rejected envelope may still have been delivered.
    pub recipients: BTreeSet<MemberId>,
    /// Who may hold its key: the authors of its entries (the creator for
    /// the first generation), its recipients, and whoever may hold a
    /// generation that descends from it.
    pub holders: BTreeSet<MemberId>,
}

/// The generation graph, by generation id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Generations(pub(crate) BTreeMap<u64, Generation>);

impl Generations {
    /// The graph over `nodes`, a set of control changes closed under their
    /// dependencies: the first generation, which `creator` holds, then the
    /// generation of every key chain entry that is valid when written,
    /// then the recipient of every key envelope for a generation the graph
    /// holds, and last who may hold each generation.
    pub(crate) fn build<'a>(
        first: u64,
        creator: MemberId,
        nodes: impl Iterator<Item = &'a Node> + Clone,
        judged: &Judged,
    ) -> Generations {
        let mut graph = BTreeMap::from([(first, Generation::default())]);
        graph.entry(first).or_default().holders.insert(creator);
        for node in nodes.clone().filter(|n| judged.valid(n.id)) {
            if let Entry::KeyChain(chain) = &node.body.entry {
                let generation = graph.entry(chain.generation).or_default();
                generation.previous.extend(chain.previous_generations());
                generation.entries.insert(node.id, node.author);
            }
        }
        for node in nodes {
            if let Entry::KeyEnvelope(envelope) = &node.body.entry
                && let Some(generation) = graph.get_mut(&envelope.generation)
            {
                generation.recipients.insert(envelope.recipient);
            }
        }
        // A generation's ordinal is one more than its previous ones', and
        // its id starts with the ordinal, so in descending order of ids
        // every generation has its holders before it hands them back.
        let ids: Vec<u64> = graph.keys().rev().copied().collect();
        for id in ids {
            let generation = graph.get_mut(&id).expect("a generation of the graph");
            let own = generation.entries.values().chain(&generation.recipients);
            let own: Vec<MemberId> = own.copied().collect();
            generation.holders.extend(own);
            let holders = generation.holders.clone();
            let previous = generation.previous.clone();
            for p in previous {
                if let Some(earlier) = graph.get_mut(&p) {
                    earlier.holders.extend(holders.iter().copied());
                }
            }
        }
        Generations(graph)
    }

    /// Whether `generation` is in the graph and is `from` or reaches it
    /// through its previous generations.
    pub(crate) fn descends(&self, generation: u64, from: u64) -> bool {
        if !self.0.contains_key(&generation) {
            return false;
        }
        let mut seen = BTreeSet::from([generation]);
        let mut stack = vec![generation];
        while let Some(id) = stack.pop() {
            if id == from {
                return true;
            }
            let previous = self.0.get(&id).into_iter().flat_map(|g| &g.previous);
            for next in previous {
                if seen.insert(*next) {
                    stack.push(*next);
                }
            }
        }
        false
    }
}
