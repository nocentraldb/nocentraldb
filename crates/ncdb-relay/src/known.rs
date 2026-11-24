//! What the relay remembers of a space between requests (NOC-427): its
//! control changes up to the last position it read, a
//! [`Resolver`] that has resolved them, and their resolve.
//!
//! Every check of a control change and every put of person-key records
//! needs RFC 0008's `resolve` of the space's control changes. Read and
//! resolved from scratch on each request, a space with many persons costs
//! a read and a resolve of all its control changes on every new person. A
//! warm process, such as a Lambda instance between invocations, now reads
//! only the control changes appended since, and keeps the change it
//! appends itself. The resolver adds a person-key entry written after the
//! whole space to the fold it holds, reading no other control change
//! (NOC-497); any other new control change is resolved with all of them,
//! and the resolver judges only the new ones.
//!
//! The relay stays stateless in what it answers: the memory holds nothing
//! the log does not, the resolver returns `resolve`'s state exactly, and
//! each request first checks that the log is still the one remembered.
//! Positions hold only within one relay epoch (RFC 0007, "Restores and the
//! relay epoch"), so a new epoch forgets every space; and the entry at the
//! last position read must still be the change remembered there, or the
//! space is read again from the start. A process without the memory, a
//! cold start, resolves the space from scratch, as before.
//!
//! The memory a space takes grows with its control changes, since the
//! resolver keeps each one's causal past as a range of node numbers
//! (NOC-442): about 7.7 KiB per control change on Linux, 75 MiB held for
//! a space of 10,000 persons, and a cold request on such a space allocates
//! at most about 104 MiB at once (NOC-492). A Lambda instance has 256 MB,
//! so the memory holds at most [`SPACES`] spaces and at most [`WEIGHT`]
//! control changes over all of them, about the memory of one space of
//! 10,000 persons: what it holds and one request's work on a space it
//! would keep take about 185 MiB, which leaves the rest to the process.
//! It forgets the spaces used least recently first, and keeps no space
//! heavier than all of it: such a space is resolved from scratch on every
//! request, as before. A request takes a space's memory out while it works
//! and puts it back when done, so two requests never share one; a second
//! request on the same space meanwhile starts from scratch.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Mutex;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{Resolved, Resolver};

use crate::Relay;
use crate::error::ErrorCode;
use crate::relay::PAGE;
use crate::store::{Entry as LogEntry, Epoch, RelayStore};

/// The most spaces the relay remembers at once.
pub const SPACES: usize = 16;

/// The most weight the memory holds: the control changes of all its
/// spaces, about 77 MiB (NOC-492). It holds a space of 10,000 persons and
/// the control changes beside them.
pub const WEIGHT: usize = 10_240;

/// A space's control changes as the relay last read them, and their
/// resolve.
pub(crate) struct Known {
    epoch: Epoch,
    pub(crate) entries: Vec<(LogEntry, Change)>,
    pub(crate) resolver: Resolver,
    pub(crate) resolved: Resolved,
}

impl Known {
    fn new(space: SpaceId, epoch: Epoch) -> Known {
        let mut resolver = Resolver::new(space);
        let resolved = resolver.resolve(std::iter::empty());
        Known {
            epoch,
            entries: Vec::new(),
            resolver,
            resolved,
        }
    }
}

/// The spaces the relay remembers, the one used last at the back.
#[derive(Default)]
pub(crate) struct Memory {
    spaces: Mutex<VecDeque<Held>>,
}

/// A space's memory and its weight.
struct Held {
    space: SpaceId,
    weight: usize,
    known: Known,
}

impl Memory {
    /// Takes the memory of `space` out, if the relay holds one.
    fn take(&self, space: SpaceId) -> Option<Known> {
        let mut spaces = self.spaces.lock().unwrap_or_else(|e| e.into_inner());
        let at = spaces.iter().position(|held| held.space == space)?;
        spaces.remove(at).map(|held| held.known)
    }

    /// Puts the memory of `space` back, as the one used last.
    fn keep(&self, space: SpaceId, known: Known) {
        let weight = known.entries.len();
        self.keep_weighed(space, known, weight);
    }

    /// Puts the memory of `space` back, as the one used last, with this
    /// weight, and forgets the spaces used least recently beyond [`SPACES`]
    /// or [`WEIGHT`]. A space heavier than [`WEIGHT`] alone is not kept.
    fn keep_weighed(&self, space: SpaceId, known: Known, weight: usize) {
        let mut spaces = self.spaces.lock().unwrap_or_else(|e| e.into_inner());
        spaces.retain(|held| held.space != space);
        if weight > WEIGHT {
            return;
        }
        spaces.push_back(Held {
            space,
            weight,
            known,
        });
        let mut total: usize = spaces.iter().map(|held| held.weight).sum();
        while spaces.len() > SPACES || total > WEIGHT {
            let Some(forgotten) = spaces.pop_front() else {
                break;
            };
            total -= forgotten.weight;
        }
    }

    /// How many spaces it holds.
    pub(crate) fn len(&self) -> usize {
        self.spaces.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl fmt::Debug for Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Memory({} spaces)", self.len())
    }
}

impl<S: RelayStore> Relay<S> {
    /// The control changes of `space` as the store holds them now, and
    /// their resolve. Give it back with [`Relay::keep`] once the request no
    /// longer changes it.
    pub(crate) async fn known(&self, space: SpaceId) -> Result<Known, ErrorCode> {
        let epoch = self.store().epoch().await?;
        let remembered = self.memory().take(space).filter(|k| k.epoch == epoch);
        if let Some(mut known) = remembered
            && self.read_new(space, &mut known).await?
        {
            return Ok(known);
        }
        let mut known = Known::new(space, epoch);
        self.read_new(space, &mut known).await?;
        Ok(known)
    }

    /// Remembers `known` for the next request on `space`.
    pub(crate) fn keep(&self, space: SpaceId, known: Known) {
        self.memory().keep(space, known);
    }

    /// Reads the control changes after the last one `known` holds, and
    /// adds the new ones to its resolve, or resolves them all when the
    /// resolver cannot add them. False, reading nothing, when the store no
    /// longer holds that last one at its position.
    async fn read_new(&self, space: SpaceId, known: &mut Known) -> Result<bool, ErrorCode> {
        let last = known
            .entries
            .last()
            .map(|(entry, _)| (entry.position, entry.change));
        let mut after = last.map_or(0, |(position, _)| position - 1);
        let mut new = Vec::new();
        let mut confirmed = last.is_none();
        loop {
            let page = self.store().read_controls(space, after, PAGE).await?;
            let Some(end) = page.last() else {
                break;
            };
            after = end.position;
            for entry in page {
                if !confirmed {
                    if Some((entry.position, entry.change)) != last {
                        return Ok(false);
                    }
                    confirmed = true;
                    continue;
                }
                // The relay appended only frames that decode.
                let change = Change::decode(&entry.frame).map_err(|_| ErrorCode::Unavailable)?;
                new.push((entry, change));
            }
        }
        if !confirmed {
            return Ok(false);
        }
        if !new.is_empty() {
            let added = known.resolver.add(new.iter().map(|(_, change)| change));
            known.entries.append(&mut new);
            known.resolved = match added {
                Some(resolved) => resolved,
                None => {
                    let changes = known.entries.iter().map(|(_, change)| change);
                    known.resolver.resolve(changes)
                }
            };
        }
        Ok(true)
    }
}

#[cfg(test)]
mod warm;

#[cfg(test)]
mod tests {
    use super::*;

    fn space(n: usize) -> SpaceId {
        let mut id = [0; 16];
        id[..8].copy_from_slice(&n.to_be_bytes());
        SpaceId(id)
    }

    #[test]
    fn the_memory_forgets_the_space_used_least_recently_beyond_its_bound() {
        let memory = Memory::default();
        let epoch = Epoch([1; 16]);
        for n in 0..=SPACES {
            memory.keep(space(n), Known::new(space(n), epoch));
        }
        assert_eq!(memory.len(), SPACES);
        assert!(
            memory.take(space(0)).is_none(),
            "the first space is forgotten"
        );
        assert!(
            memory.take(space(SPACES)).is_some(),
            "the last space is held"
        );
    }

    #[test]
    fn the_memory_forgets_spaces_used_least_recently_until_their_weight_fits() {
        let memory = Memory::default();
        let epoch = Epoch([1; 16]);
        let third = WEIGHT / 3 + 1;
        for n in 0..3 {
            memory.keep_weighed(space(n), Known::new(space(n), epoch), third);
        }
        assert_eq!(memory.len(), 2, "three thirds and more do not fit");
        assert!(
            memory.take(space(0)).is_none(),
            "the first space is forgotten"
        );
    }

    #[test]
    fn a_space_heavier_than_the_whole_weight_is_not_kept() {
        let memory = Memory::default();
        let epoch = Epoch([1; 16]);
        memory.keep_weighed(space(0), Known::new(space(0), epoch), 1);
        memory.keep_weighed(space(1), Known::new(space(1), epoch), WEIGHT + 1);
        assert!(
            memory.take(space(1)).is_none(),
            "the heavy space is not kept"
        );
        assert!(memory.take(space(0)).is_some(), "the others stay");
    }

    #[test]
    fn a_space_taken_out_and_kept_again_counts_as_used_last() {
        let memory = Memory::default();
        let epoch = Epoch([1; 16]);
        for n in 0..SPACES {
            memory.keep(space(n), Known::new(space(n), epoch));
        }
        let first = memory.take(space(0)).expect("held");
        memory.keep(space(0), first);
        memory.keep(space(SPACES), Known::new(space(SPACES), epoch));
        assert!(
            memory.take(space(0)).is_some(),
            "the space used last is held"
        );
        assert!(
            memory.take(space(1)).is_none(),
            "the one used least recently is not"
        );
    }
}
