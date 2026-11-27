//! What the log keeps of each change (NOC-452, NOC-518): its id, the
//! fields of its header the log and the checks read, its state and the
//! relay's answer, and the change itself only while the log holds it in
//! memory. An author is kept once, in [`Authors`], and an entry names it
//! by its number there, so an entry of a released change takes about 150
//! bytes, whatever the change's size.

use std::collections::BTreeMap;

use super::state::{RelayRefusal, State};
use crate::change::{Change, HeaderFields, Kind};
use crate::ids::{ChangeId, MemberId, SpaceId};

/// One entry: the change's id and header fields, the change while the log
/// holds it, and on a member its state, whether the relay confirmed it,
/// the relay's refusal and when the member wrote it.
#[derive(Clone)]
pub(super) struct Entry {
    pub(super) id: ChangeId,
    /// The change, until the log releases it; a control change always.
    pub(super) held: Option<Box<Change>>,
    /// The author's number in the log's [`Authors`].
    pub(super) author: Author,
    pub(super) sequence: u64,
    pub(super) key_generation: u64,
    pub(super) kind: Kind,
    pub(super) format: u64,
    pub(super) suite: u64,
    pub(super) state: Option<State>,
    pub(super) confirmed: bool,
    /// Boxed, since few changes have one.
    pub(super) refusal: Option<Box<RelayRefusal>>,
    /// When the member wrote the change, for one it wrote.
    pub(super) written_at: Option<u64>,
}

impl Entry {
    /// The entry of a change just appended, by `author`.
    pub(super) fn new(change: Change, author: Author) -> Entry {
        let header = change.header();
        Entry {
            id: change.id(),
            author,
            sequence: header.sequence,
            key_generation: header.key_generation,
            kind: header.kind,
            format: header.format,
            suite: header.suite,
            held: Some(Box::new(change)),
            state: None,
            confirmed: false,
            refusal: None,
            written_at: None,
        }
    }

    /// The change, while the log holds it in memory.
    pub(super) fn held(&self) -> Option<&Change> {
        self.held.as_deref()
    }

    /// The fields of the change's header besides its payload and its
    /// unknown keys.
    pub(super) fn fields(&self, space: SpaceId, authors: &Authors) -> HeaderFields {
        HeaderFields {
            space,
            author: authors.id(self.author),
            sequence: self.sequence,
            key_generation: self.key_generation,
            kind: self.kind,
            format: self.format,
            suite: self.suite,
        }
    }
}

/// The number of an author in a log's [`Authors`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Author(u32);

impl Author {
    /// The author's place among the log's authors, from 0.
    pub(super) fn index(self) -> usize {
        self.0 as usize
    }
}

/// The authors of a log's changes, each kept once and numbered in the
/// order they first appear.
#[derive(Clone, Default)]
pub(super) struct Authors {
    ids: Vec<MemberId>,
    numbers: BTreeMap<MemberId, Author>,
}

impl Authors {
    /// The number of `id`, which it gets now if it has none yet.
    pub(super) fn number(&mut self, id: MemberId) -> Author {
        if let Some(author) = self.numbers.get(&id) {
            return *author;
        }
        let author =
            Author(u32::try_from(self.ids.len()).expect("a log has fewer than 2^32 authors"));
        self.ids.push(id);
        self.numbers.insert(id, author);
        author
    }

    /// The id of the author numbered `author`.
    pub(super) fn id(&self, author: Author) -> MemberId {
        self.ids[author.index()]
    }
}
