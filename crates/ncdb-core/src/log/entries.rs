//! The entries of a log, in chunks of at most [`CHUNK`] entries. A log of
//! many changes so never holds more than one chunk of unused room, where
//! one vector of all entries could hold as much room again as it uses
//! after it grew, and never copies all entries to grow (NOC-518). A small
//! log's one chunk grows as a vector does.

use std::ops::{Index, IndexMut};

use super::entry::Entry;

/// The most entries in one chunk.
const CHUNK: usize = 1024;

#[derive(Clone, Default)]
pub(super) struct Entries {
    chunks: Vec<Vec<Entry>>,
    len: usize,
}

impl Entries {
    pub(super) fn push(&mut self, entry: Entry) {
        match self.chunks.last_mut() {
            Some(chunk) if chunk.len() < CHUNK => chunk.push(entry),
            _ => {
                let mut chunk = Vec::new();
                if !self.chunks.is_empty() {
                    chunk.reserve_exact(CHUNK);
                }
                chunk.push(entry);
                self.chunks.push(chunk);
            }
        }
        self.len += 1;
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) fn get(&self, index: usize) -> Option<&Entry> {
        self.chunks.get(index / CHUNK)?.get(index % CHUNK)
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut Entry> {
        self.chunks.get_mut(index / CHUNK)?.get_mut(index % CHUNK)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &Entry> {
        self.chunks.iter().flatten()
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Entry> {
        self.chunks.iter_mut().flatten()
    }
}

impl Index<usize> for Entries {
    type Output = Entry;

    fn index(&self, index: usize) -> &Entry {
        self.get(index).expect("an index the entries hold")
    }
}

impl IndexMut<usize> for Entries {
    fn index_mut(&mut self, index: usize) -> &mut Entry {
        self.get_mut(index).expect("an index the entries hold")
    }
}
