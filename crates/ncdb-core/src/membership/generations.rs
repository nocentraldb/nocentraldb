//! The generation graph of RFC 0008 ("Generation ids"): the space's first
//! generation, and every generation a key chain entry that is valid when
//! written names, accepted or later rejected (decision M17), each linked to
//! its previous generations. "Descends from" follows those links.

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
}

/// The generation graph, by generation id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Generations(pub(crate) BTreeMap<u64, Generation>);

impl Generations {
    /// The graph over `nodes`, a set of control changes closed under their
    /// dependencies: the first generation, then the generation of every
    /// key chain entry that is valid when written, then the recipient of
    /// every key envelope for a generation the graph holds.
    pub(crate) fn build<'a>(
        first: u64,
        nodes: impl Iterator<Item = &'a Node> + Clone,
        judged: &Judged,
    ) -> Generations {
        let mut graph = BTreeMap::from([(first, Generation::default())]);
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
