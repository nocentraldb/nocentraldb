//! The current generation of RFC 0008 ("Two removals that rotate the key at
//! once", step 3): the generation members write sealed changes with.
//!
//! It is a head of the generation graph, a generation that no accepted key
//! chain entry lists as previous. Among the heads it is the one with the
//! highest ordinal, then the one whose key chain entry comes first in the
//! order. A generation that two key chain entries name has two keys under
//! one id (RFC 0008, "Generation ids"), so it is never a head, and an entry that names it
//! lists nothing.
//!
//! When the current generation is exposed, an admin writes a merge
//! rotation ([`MergeRotation`]), step 5 of the same section.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::SpaceKey;

use super::body::Entry;
use super::generations::Generations;
use super::genesis::generation_id;
use super::input::Node;
use super::key_chain::{KeyChainEntry, PreviousKey, seal_previous_key};
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

/// The merge rotation an admin writes when the current generation is
/// exposed (RFC 0008, "Two removals that rotate the key at once", step 5):
/// a key chain entry that lists every head as previous, and envelopes for
/// the current members only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeRotation {
    /// The space it rotates.
    pub space: SpaceId,
    /// The new generation id, derived from the space, the admin and the
    /// heads (RFC 0008, "Generation ids").
    pub generation: u64,
    /// Every head, sorted by generation id.
    pub previous: Vec<u64>,
    /// The current members, the admin included: one envelope each.
    pub recipients: BTreeSet<MemberId>,
}

impl MergeRotation {
    pub(crate) fn new(
        space: SpaceId,
        author: MemberId,
        heads: &BTreeSet<u64>,
        recipients: BTreeSet<MemberId>,
    ) -> MergeRotation {
        let previous: Vec<u64> = heads.iter().copied().collect();
        MergeRotation {
            space,
            generation: generation_id(space, author, &previous),
            previous,
            recipients,
        }
    }

    /// Its key chain entry under `new_key`, a fresh key for the new
    /// generation: each head's key, which `key` finds, encrypted in
    /// NOC-97's layout ([`seal_previous_key`]). None when `key` lacks the
    /// key of a head: an admin that does not hold every head cannot merge
    /// them. A merge rotation answers no revocation.
    pub fn entry<'k>(
        &self,
        new_key: &SpaceKey,
        mut key: impl FnMut(u64) -> Option<&'k SpaceKey>,
    ) -> Option<KeyChainEntry> {
        let previous = self.previous.iter().map(|generation| {
            let sealed = seal_previous_key(
                new_key,
                self.space,
                self.generation,
                *generation,
                key(*generation)?,
            );
            Some(PreviousKey {
                generation: *generation,
                key: sealed,
            })
        });
        Some(KeyChainEntry {
            generation: self.generation,
            previous: previous.collect::<Option<_>>()?,
            answers: BTreeSet::new(),
        })
    }
}
