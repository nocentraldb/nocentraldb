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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::change::{Change, Kind};
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
    /// not know yet are parked on their own (D10), listed here in order. A
    /// sealed change moves on to rejected, and is un-folded, when a later
    /// control change makes its membership check fail (RFC 0006,
    /// "Un-folding").
    Folded {
        /// The operations of the change that were not folded.
        parked: Vec<ParkedOperation>,
    },
    /// Kept and not folded, because this client does not understand it
    /// yet; folded after an upgrade.
    Parked(ParkReason),
    /// Kept and not folded, because it failed a check, or un-folded,
    /// because a later control change made it fail; checked again when a
    /// control change of the space passes (D11).
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
    /// Why it was parked.
    pub reason: OperationParkReason,
}

/// Why an operation is parked on its own: step 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum OperationParkReason {
    /// The member's model does not know the collection.
    UnknownCollection,
    /// The member's model does not know the field.
    UnknownField,
    /// The operation carries an unknown "must understand" key.
    UnknownKey,
    /// The field's type does not define the operation's number (RFC 0004,
    /// Amendment 1).
    UnknownOperation,
    /// The operation's arguments carry a "must understand" key, 4 to 63,
    /// that the operation does not define (RFC 0004, Amendment 1).
    UnknownArgument,
    /// The operation's arguments are malformed (RFC 0004, Amendment 1,
    /// decision 12).
    MalformedArguments,
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

/// The relay's refusal of a change the member holds: the relay's error
/// code (RFC 0007), such as `author_not_member`. It is recorded beside the
/// change and never changes its [`State`] (D12): no other member sees the
/// relay's answer, so a state that followed it could differ between
/// members that hold the same changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayRefusal(pub String);

/// One entry: the change, and on a member its state and the relay's
/// refusal.
#[derive(Clone)]
struct Entry {
    change: Change,
    state: Option<State>,
    refusal: Option<RelayRefusal>,
}

/// The log of one space. Its `Debug` output shows the space and the number
/// of entries.
#[derive(Clone)]
pub struct Log {
    space: SpaceId,
    entries: Vec<Entry>,
    positions: BTreeMap<ChangeId, Position>,
    /// The changes whose state was set since [`Log::take_changed`] last
    /// ran, for a store that keeps the states.
    changed: BTreeSet<ChangeId>,
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
            changed: BTreeSet::new(),
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
            refusal: None,
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

    /// The relay's refusal of a change, when one was recorded.
    pub fn relay_refusal(&self, id: ChangeId) -> Option<&RelayRefusal> {
        self.entry(id)?.refusal.as_ref()
    }

    /// Records the relay's refusal of a change the log holds, beside the
    /// change; its state stays as it is (D12). Returns `false`, and records
    /// nothing, when the log does not hold the change.
    pub fn record_relay_refusal(&mut self, id: ChangeId, refusal: RelayRefusal) -> bool {
        match self.entry_mut(id) {
            Some(entry) => {
                entry.refusal = Some(refusal);
                true
            }
            None => false,
        }
    }

    /// Sets the state of a change the log holds.
    pub(crate) fn set_state(&mut self, id: ChangeId, state: State) {
        if let Some(entry) = self.entry_mut(id) {
            entry.state = Some(state);
            self.changed.insert(id);
        }
    }

    /// The changes whose state was set since the last call, which a store
    /// of the states must write.
    pub(crate) fn take_changed(&mut self) -> BTreeSet<ChangeId> {
        std::mem::take(&mut self.changed)
    }

    /// The rejected changes, in log order.
    pub(crate) fn rejected(&self) -> Vec<Change> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.state, Some(State::Rejected(_))))
            .map(|entry| entry.change.clone())
            .collect()
    }

    /// The control changes that passed their checks, in log order.
    pub(crate) fn folded_controls(&self) -> impl Iterator<Item = &Change> {
        self.entries
            .iter()
            .filter(|entry| {
                entry.change.header().kind == Kind::CONTROL
                    && matches!(entry.state, Some(State::Folded { .. }))
            })
            .map(|entry| &entry.change)
    }

    /// The folded changes that are not control changes, in log order, with
    /// their parked operations.
    pub(crate) fn folded_sealed(&self) -> Vec<(Change, Vec<ParkedOperation>)> {
        self.entries
            .iter()
            .filter(|entry| entry.change.header().kind != Kind::CONTROL)
            .filter_map(|entry| match &entry.state {
                Some(State::Folded { parked }) => Some((entry.change.clone(), parked.clone())),
                _ => None,
            })
            .collect()
    }

    fn entry(&self, id: ChangeId) -> Option<&Entry> {
        self.entries.get(self.index_of(id)?)
    }

    fn entry_mut(&mut self, id: ChangeId) -> Option<&mut Entry> {
        let index = self.index_of(id)?;
        self.entries.get_mut(index)
    }

    fn index_of(&self, id: ChangeId) -> Option<usize> {
        usize::try_from(self.position_of(id)?.0.checked_sub(1)?).ok()
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
