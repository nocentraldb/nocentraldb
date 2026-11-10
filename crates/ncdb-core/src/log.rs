//! An append-only log of one space, in memory (RFC 0002, "The log on
//! disk").
//!
//! Entries get dense positions 1, 2, 3, ... in the order they are appended,
//! and keep them. The log never changes or removes an entry, and holds each
//! change id once. A cursor is a position: the entries after it are handed
//! out in order. Persistence to SQLite is a later ticket.
//!
//! On a member, each entry also carries its [`State`]: folded, parked or
//! rejected (RFC 0002, "Rejected changes"), and the relay's refusal when there is
//! one, beside the frame bytes. [`Log::receive`](crate::receive) sets both.
//! The relay's log holds no states: an entry appended with [`Log::append`]
//! has none.
//!
//! The log also keeps its [`Tips`] as it appends changes and sets their
//! states: what the member's next change takes from it (NOC-138).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::change::{Change, Kind};
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::receive::Checks;
use crate::tips::{Opened, Tips};

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

/// Where a member's check of a change stopped (RFC 0002, "What
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
    /// Payload key 64 names another model than the member's lock, or has
    /// the wrong shape: the change is parked whole (RFC 0002, "The
    /// writer's model").
    AnotherModel,
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
    /// "Operations a member does not understand").
    UnknownOperation,
    /// The operation's arguments carry a "must understand" key, 4 to 63,
    /// that the operation does not define (RFC 0004, "Operations a member
    /// does not understand").
    UnknownArgument,
    /// The operation's arguments are malformed (RFC 0004, "The
    /// arguments of each operation", decision 12).
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

/// How many of one author's changes in a log the relay has neither
/// confirmed nor refused, and how many it refused (NOC-147). A refused
/// change counts as refused alone, even when the relay also confirmed it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelayCounts {
    /// The changes with neither a relay position nor a refusal.
    pub unconfirmed: u64,
    /// The changes the relay refused.
    pub refused: u64,
}

/// One entry: the change, and on a member its state, whether the relay
/// confirmed it and the relay's refusal.
#[derive(Clone)]
struct Entry {
    change: Change,
    state: Option<State>,
    confirmed: bool,
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
    tips: Tips,
    /// The relay counts of each author, kept as changes are appended,
    /// confirmed and refused, so reading them costs no pass over the log.
    relay_counts: BTreeMap<MemberId, RelayCounts>,
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
            tips: Tips::default(),
            relay_counts: BTreeMap::new(),
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
        self.tips.appended(&change);
        self.counts_mut(change.header().author).unconfirmed += 1;
        self.entries.push(Entry {
            change,
            state: None,
            confirmed: false,
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
        let Some(entry) = self.entry_mut(id) else {
            return false;
        };
        let newly = entry.refusal.replace(refusal).is_none();
        let (author, confirmed) = (entry.change.header().author, entry.confirmed);
        if newly {
            let counts = self.counts_mut(author);
            counts.refused += 1;
            if !confirmed {
                counts.unconfirmed -= 1;
            }
        }
        true
    }

    /// Forgets the relay's refusal of a change the log holds, so that the
    /// member pushes it to the relay once more (RFC 0008, M19); a change
    /// the relay has not confirmed counts as unconfirmed again. Returns
    /// `false`, and changes nothing, when the log does not hold the change
    /// or holds no refusal of it.
    pub fn clear_relay_refusal(&mut self, id: ChangeId) -> bool {
        let Some(entry) = self.entry_mut(id) else {
            return false;
        };
        if entry.refusal.take().is_none() {
            return false;
        }
        let (author, confirmed) = (entry.change.header().author, entry.confirmed);
        let counts = self.counts_mut(author);
        counts.refused -= 1;
        if !confirmed {
            counts.unconfirmed += 1;
        }
        true
    }

    /// Records that the relay holds a change the log holds, at a position
    /// the member keeps elsewhere. Returns `false`, and records nothing,
    /// when the log does not hold the change.
    pub fn record_relay_confirmation(&mut self, id: ChangeId) -> bool {
        let Some(entry) = self.entry_mut(id) else {
            return false;
        };
        let newly = !std::mem::replace(&mut entry.confirmed, true);
        let (author, refused) = (entry.change.header().author, entry.refusal.is_some());
        if newly && !refused {
            self.counts_mut(author).unconfirmed -= 1;
        }
        true
    }

    /// Forgets every confirmation of the relay, whose epoch changed;
    /// refusals stay (RFC 0007, "Restores and the relay epoch").
    pub fn forget_relay_confirmations(&mut self) {
        let mut returned = Vec::new();
        for entry in &mut self.entries {
            if std::mem::take(&mut entry.confirmed) && entry.refusal.is_none() {
                returned.push(entry.change.header().author);
            }
        }
        for author in returned {
            self.counts_mut(author).unconfirmed += 1;
        }
    }

    /// The relay counts of `author`'s changes in this log.
    pub fn relay_counts(&self, author: MemberId) -> RelayCounts {
        self.relay_counts.get(&author).copied().unwrap_or_default()
    }

    fn counts_mut(&mut self, author: MemberId) -> &mut RelayCounts {
        self.relay_counts.entry(author).or_default()
    }

    /// Sets the state of a change the log holds.
    pub(crate) fn set_state(&mut self, id: ChangeId, state: State) {
        self.set_state_opened(id, state, None);
    }

    /// Sets the state of a change the log holds, with what its payload
    /// gave when the checks opened it, and keeps the tips.
    pub(crate) fn set_state_opened(&mut self, id: ChangeId, state: State, opened: Option<Opened>) {
        let Some(index) = self.index_of(id) else {
            return;
        };
        let entry = &mut self.entries[index];
        let folded = |state: &Option<State>| matches!(state, Some(State::Folded { .. }));
        if folded(&entry.state) {
            self.tips.unfolded(id);
        }
        entry.state = Some(state);
        if folded(&entry.state) {
            self.tips.folded(&entry.change, opened);
        }
        self.changed.insert(id);
    }

    /// The heads among the folded control changes, which the member's next
    /// control change depends on (RFC 0008). A control change's
    /// dependencies are in its header, so this opens no change.
    pub fn control_heads(&self) -> &BTreeSet<ChangeId> {
        self.tips.control_heads()
    }

    /// The tips of the log: what the member's next change takes from it.
    /// A folded sealed change whose state was read back from storage, and
    /// so whose payload the log has not read, is opened with `checks`
    /// once, here; one that does not open is left out until it does.
    pub fn tips(&mut self, checks: &impl Checks) -> &Tips {
        for id in self.tips.unread().clone() {
            let change = &self.entries[self.index_of(id).expect("the log holds the change")].change;
            if let Ok(payload) = checks.open(change) {
                let opened = Opened {
                    dependencies: payload.dependencies,
                    clock: payload.clock,
                };
                self.tips.read(id, opened);
            }
        }
        &self.tips
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
