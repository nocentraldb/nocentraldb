//! The changes of each author and sequence in a log, kept as changes are
//! appended; two or more are an equivocation (RFC 0002, "Author
//! sequence"). An author's changes mostly arrive in sequence order, so the
//! first change of each sequence from 1 up without a gap is kept in a list
//! per author, by its position alone, and only a sequence after a gap in a
//! map. The later changes of an equivocation are kept in a list each. A
//! change so costs one position (NOC-518).

use std::collections::BTreeMap;

use super::Position;
use super::entry::Author;

/// An author and one of its sequences.
type Key = (Author, u64);

#[derive(Clone, Default)]
pub(super) struct Sequences {
    /// Per author, by its number: the first change of each sequence from
    /// 1 up to the first gap, at the sequence's index less one.
    dense: Vec<Vec<Position>>,
    /// The first change of each author and sequence after a gap.
    sparse: BTreeMap<Key, Position>,
    /// The later changes of the authors and sequences with two or more,
    /// in the order they were appended.
    later: BTreeMap<Key, Vec<Position>>,
}

impl Sequences {
    /// Adds the change at `position`, of `author` and `sequence`.
    pub(super) fn add(&mut self, author: Author, sequence: u64, position: Position) {
        let key = (author, sequence);
        if self.first(key).is_some() {
            self.later.entry(key).or_default().push(position);
            return;
        }
        if self.dense.len() <= author.index() {
            self.dense.resize_with(author.index() + 1, Vec::new);
        }
        let dense = &mut self.dense[author.index()];
        if sequence != next(dense) {
            self.sparse.insert(key, position);
            return;
        }
        dense.push(position);
        while let Some(position) = self.sparse.remove(&(author, next(dense))) {
            dense.push(position);
        }
    }

    /// The first change of an author and sequence.
    fn first(&self, (author, sequence): Key) -> Option<Position> {
        let dense = self.dense.get(author.index())?;
        let index = sequence
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok());
        match index.and_then(|index| dense.get(index)) {
            Some(position) => Some(*position),
            None => self.sparse.get(&(author, sequence)).copied(),
        }
    }

    /// The positions of every change of an equivocation, in no order.
    pub(super) fn equivocating(&self) -> impl Iterator<Item = Position> + '_ {
        self.later.iter().flat_map(|(key, later)| {
            let first = self
                .first(*key)
                .expect("an equivocation has a first change");
            std::iter::once(first).chain(later.iter().copied())
        })
    }
}

/// The sequence after the ones `dense` holds.
fn next(dense: &[Position]) -> u64 {
    u64::try_from(dense.len()).expect("a length fits 64 bits") + 1
}
