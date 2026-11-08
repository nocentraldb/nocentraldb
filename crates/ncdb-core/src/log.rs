//! An append-only log of one space, in memory (RFC 0002, "The log on
//! disk").
//!
//! Entries get dense positions 1, 2, 3, ... in the order they are appended,
//! and keep them. The log never changes or removes an entry, and holds each
//! change id once. A cursor is a position: the entries after it are handed
//! out in order. Persistence to SQLite is a later ticket.
//!
//! On a member, each entry also carries its [`State`]: folded, parked or
//! rejected (RFC 0002 Amendment 1), and the relay's refusal when there is
//! one, beside the frame bytes. [`Log::receive`](crate::receive) sets both.
//! The relay's log holds no states: an entry appended with [`Log::append`]
//! has none.

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppendError::WrongSpace => f.write_str("the change belongs to another space"),
        }
    }
}

impl std::error::Error for AppendError {}

/// Where a member's check of a change stopped (RFC 0002 Amendment 1, "What
/// a member does with a change it receives"). New states and reasons may
/// follow, so the enums are open to extension.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum State {
    /// Passed every check and folded. Operations the member's model does
    /// not know yet are parked on their own (D10), listed here in order.
    Folded {
        /// The operations of the change that were not folded.
        parked: Vec<ParkedOperation>,
    },
    /// Kept and not folded, because this client does not understand it
    /// yet; folded after an upgrade.
    Parked(ParkReason),
    /// Kept and not folded, because it failed a check; checked again when a
    /// control change of the space arrives (D11).
    Rejected(RejectReason),
}

/// Why a change is parked: steps 2 and 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParkReason {
    /// The suite id is unknown.
    UnknownSuite,
    /// The kind is unknown.
    UnknownKind,
    /// The header format is unknown.
    UnknownHeaderFormat,
    /// The header carries an unknown "must understand" key.
    UnknownHeaderKey,
    /// The payload format is unknown, or the payload is not a map this
    /// core can read, as a newer format may not be.
    UnknownPayloadFormat,
    /// The payload carries an unknown "must understand" key.
    UnknownPayloadKey,
}

/// An operation of a folded change that was parked on its own (D10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParkedOperation {
    /// The operation's position in the change's operation array.
    pub position: u64,
}

/// Why a change is rejected: steps 3 to 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RejectReason {
    /// Step 3: no membership entry the member holds names the author.
    UnknownAuthor,
    /// Step 3: the signature does not verify.
    BadSignature,
    /// Step 4: the author is not a member at the change's key generation.
    NotMember,
    /// Step 5: the member holds no key envelope for the key generation.
    NoKey,
    /// Step 5: the AEAD fails.
    Undecryptable,
}

/// One entry: the change, and on a member its state.
#[derive(Clone)]
struct Entry {
    change: Change,
    state: Option<State>,
}

/// The log of one space. Its `Debug` output shows the space and the number
/// of entries.
#[derive(Clone)]
pub struct Log {
    space: SpaceId,
    entries: Vec<Entry>,
    positions: BTreeMap<ChangeId, Position>,
}

impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Log")
            .field("space", &self.space)
            .field("entries", &self.entries.len())
            .finish()
    }
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
    pub fn append(&mut self, change: Change) -> Result<Appended, AppendError> {
        if change.header().space != self.space {
            return Err(AppendError::WrongSpace);
        }
        let id = change.id();
        if let Some(position) = self.position_of(id) {
            return Ok(Appended::Duplicate(position));
        }
        self.entries.push(Entry {
            change,
            state: None,
        });
        let position = self.head();
        self.positions.insert(id, position);
        Ok(Appended::New(position))
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The position of the last entry, or [`Position::START`].
    pub fn head(&self) -> Position {
        Position(u64::try_from(self.entries.len()).expect("a length fits 64 bits"))
    }

    /// The entry at a position.
    pub fn get(&self, position: Position) -> Option<&Change> {
        let index = usize::try_from(position.0.checked_sub(1)?).ok()?;
        self.entries.get(index).map(|entry| &entry.change)
    }

    /// The member's state of a change, when the log holds it with one.
    pub fn state(&self, id: ChangeId) -> Option<&State> {
        self.entry(id)?.state.as_ref()
    }

    /// Sets the state of a change the log holds.
    pub(crate) fn set_state(&mut self, id: ChangeId, state: State) {
        if let Some(position) = self.position_of(id) {
            let index = usize::try_from(position.0 - 1).expect("a position fits usize");
            self.entries[index].state = Some(state);
        }
    }

    fn entry(&self, id: ChangeId) -> Option<&Entry> {
        let index = usize::try_from(self.position_of(id)?.0 - 1).ok()?;
        self.entries.get(index)
    }

    /// The position of a change, when the log holds it.
    pub fn position_of(&self, id: ChangeId) -> Option<Position> {
        self.positions.get(&id).copied()
    }

    /// The entries after a cursor, in order, with their positions.
    pub fn after(&self, cursor: Position) -> impl Iterator<Item = (Position, &Change)> {
        let skip = usize::try_from(cursor.0).unwrap_or(usize::MAX);
        self.entries
            .iter()
            .zip(1..)
            .skip(skip)
            .map(|(entry, position)| (Position(position), &entry.change))
    }
}
