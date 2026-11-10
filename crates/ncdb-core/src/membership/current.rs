//! The current generation of RFC 0008 ("Two removals that rotate the key at
//! once", step 3): the generation members write sealed changes with.
//!
//! It is a head of the generation graph, a generation that no accepted key
//! chain entry lists as previous. Among the heads it is the one with the
//! highest ordinal, then the one whose key chain entry comes first in the
//! order. A generation that two key chain entries name has two keys under
//! one id (decision M23), so it is never a head, and an entry that names it
//! lists nothing.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::ChangeId;

use super::body::Entry;
use super::generations::Generations;
use super::input::Node;
use super::state::Verdict;

/// The ordinal of a generation id: its upper 16 bits (RFC 0008,
/// "Generation ids").
pub(crate) fn ordinal(generation: u64) -> u64 {
    generation >> 48
}

/// The heads of the graph: every generation named by at most one key chain
/// entry that no accepted entry of such a generation lists as previous.
pub(crate) fn heads(
    generations: &Generations,
    verdicts: &BTreeMap<ChangeId, Verdict>,
    nodes: &BTreeMap<ChangeId, &Node>,
) -> BTreeSet<u64> {
    let unambiguous = |id: &u64| generations.0.get(id).is_some_and(|g| g.entries.len() <= 1);
    let mut heads: BTreeSet<u64> = generations.0.keys().copied().filter(unambiguous).collect();
    for (id, node) in nodes {
        if let Entry::KeyChain(chain) = &node.body.entry
            && verdicts.get(id) == Some(&Verdict::Accepted)
            && unambiguous(&chain.generation)
        {
            for previous in chain.previous_generations() {
                heads.remove(&previous);
            }
        }
    }
    heads
}

/// The current generation among `heads`: the highest ordinal, then the
/// head whose key chain entry comes first in `order`.
pub(crate) fn current(
    heads: &BTreeSet<u64>,
    generations: &Generations,
    order: &[ChangeId],
) -> Option<u64> {
    let position: BTreeMap<ChangeId, usize> =
        order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let first_in_order = |generation: &u64| {
        let entries = generations
            .0
            .get(generation)
            .into_iter()
            .flat_map(|g| g.entries.keys());
        entries.filter_map(|id| position.get(id)).min().copied()
    };
    heads
        .iter()
        .copied()
        .max_by_key(|g| (ordinal(*g), std::cmp::Reverse(first_in_order(g))))
}
