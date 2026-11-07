//! An append-only log of one space, in memory (RFC 0002, "The log on
//! disk").
//!
//! Entries get dense positions 1, 2, 3, ... in the order they are appended,
//! and keep them. The log never changes or removes an entry, and holds each
//! change id once. A cursor is a position: the entries after it are handed
//! out in order. Persistence to SQLite is a later ticket.

use std::collections::BTreeMap;
use std::fmt;

use crate::change::Change;
use crate::ids::{ChangeId, SpaceId};

/// A position in the log: 1 for the first entry. A cursor at position `p`
/// hands out the entries after `p`; [`Position::START`] hands out all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position(pub u64);

impl Position {
    /// Before the first entry.
    pub const START: Position = Position(0);
}

/// What appending a change did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appended {
    /// The change is new, at this position.
    New(Position),
    /// The log already held the change, at this position; nothing changed.
    Duplicate(Position),
}

/// Why a change was not appended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppendError {
    /// The change belongs to another space.
    WrongSpace,
}

impl fmt::Display for AppendError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for AppendError {}

/// The log of one space.
#[derive(Clone, Debug)]
pub struct Log {
    space: SpaceId,
    entries: Vec<Change>,
    positions: BTreeMap<ChangeId, Position>,
}

impl Log {
    /// An empty log of a space.
    pub fn new(space: SpaceId) -> Log {
        Log {
            space,
            entries: Vec::new(),
            positions: BTreeMap::new(),
        }
    }

    /// The space this log belongs to.
    pub fn space(&self) -> SpaceId {
        self.space
    }

    /// Appends a change of this space, unless the log already holds it.
    pub fn append(&mut self, _change: Change) -> Result<Appended, AppendError> {
        todo!()
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        todo!()
    }

    /// Whether the log has no entries.
    pub fn is_empty(&self) -> bool {
        todo!()
    }

    /// The position of the last entry, or [`Position::START`].
    pub fn head(&self) -> Position {
        todo!()
    }

    /// The entry at a position.
    pub fn get(&self, _position: Position) -> Option<&Change> {
        todo!()
    }

    /// The position of a change, when the log holds it.
    pub fn position_of(&self, _id: ChangeId) -> Option<Position> {
        todo!()
    }

    /// The entries after a cursor, in order, with their positions.
    pub fn after(&self, _cursor: Position) -> impl Iterator<Item = (Position, &Change)> {
        let _ = (&self.entries, &self.positions);
        std::iter::empty::<(Position, &Change)>()
    }
}
