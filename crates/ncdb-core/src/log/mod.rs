//! An append-only log of one space (RFC 0002, "The log on disk").
//!
//! Entries get dense positions 1, 2, 3, ... in the order they are appended,
//! and keep them. The log never changes or removes an entry, and holds each
//! change id once. A cursor is a position: the entries after it are handed
//! out in order.
//!
//! On a member, each entry also carries its [`State`]: folded, parked or
//! rejected (RFC 0002, "Rejected changes"), and the relay's refusal when there is
//! one. [`Log::receive`](crate::receive) sets both.
//! The relay's log holds no states: an entry appended with [`Log::append`]
//! has none.
//!
//! The log also keeps its [`Tips`] as it appends changes and sets their
//! states: what the member's next change takes from it (NOC-138).
//!
//! What the log holds in memory (NOC-452): for every change its id, the
//! fields of its header besides the payload, its state, the relay's answer
//! and when the member wrote it, and beside them the positions, the
//! sequences, the tips, the counts and the reports a space's status reads. Every control change stays in
//! memory whole: the membership checks fold all of them, and a control
//! change is small. A sealed change stays in memory only until the store
//! that keeps the log's frames, such as the view's `log` table, has it;
//! [`Log::release`] then drops its bytes, and a pass that needs them reads
//! the change back through [`Frames`]. Such passes are rare: checking a
//! rejected change again, un-folding, folding a field again, reading a
//! move's later edits, and the tips once after a reopen. The memory of a
//! log so grows with its number of changes, not with their bytes: each
//! change costs one entry of about 150 bytes, which names its author by a
//! number, its position, its author sequence, and its node among the
//! tips' heads with its dependencies as a slice, about 400 bytes in all
//! (NOC-518).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

mod entries;
mod entry;
mod frames;
mod sequences;
mod state;

pub use frames::{Frames, Held, NoFrames};

pub use state::{
    OperationParkReason, ParkReason, ParkedOperation, RejectReason, RelayCounts, RelayRefusal,
    State, StateCounts,
};

use crate::change::{Change, Header, Kind};
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::receive::Checks;
use crate::texts;
use crate::tips::{Opened, Tips};
use entries::Entries;
use entry::{Authors, Entry};
use sequences::Sequences;

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
            AppendError::WrongSpace => f.write_str(texts::APPEND_WRONG_SPACE),
        }
    }
}

impl std::error::Error for AppendError {}

/// The log of one space. Its `Debug` output shows the space and the number
/// of entries.
#[derive(Clone)]
pub struct Log {
    space: SpaceId,
    entries: Entries,
    authors: Authors,
    positions: BTreeMap<ChangeId, Position>,
    /// The changes whose state was set since [`Log::take_changed`] last
    /// ran, for a store that keeps the states.
    changed: BTreeSet<ChangeId>,
    tips: Tips,
    /// The relay counts of each author, kept as changes are appended,
    /// confirmed and refused, so reading them costs no pass over the log.
    relay_counts: BTreeMap<MemberId, RelayCounts>,
    /// The parked and rejected counts, kept as states are set.
    state_counts: StateCounts,
    /// The changes of each author and sequence, kept as changes are
    /// appended. This, `refused` and `parked` let a space's status, which
    /// the SDK reads after every write, be read without a pass over the
    /// log (NOC-439).
    sequences: Sequences,
    /// The positions of each author's changes the relay refused, kept as
    /// refusals are recorded and forgotten.
    refused: BTreeMap<MemberId, BTreeSet<Position>>,
    /// The positions of the changes with parked data: parked for an
    /// unknown "must understand" payload key, or folded with parked
    /// operations; kept as states are set.
    parked: BTreeSet<Position>,
    /// The changes the member wrote that the relay has not confirmed, by
    /// the time they were written, kept as times are recorded and the
    /// relay confirms; a refused change counts, since only this device
    /// holds it.
    unconfirmed_written: BTreeSet<(u64, Position)>,
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
            entries: Entries::default(),
            authors: Authors::default(),
            positions: BTreeMap::new(),
            changed: BTreeSet::new(),
            tips: Tips::default(),
            relay_counts: BTreeMap::new(),
            state_counts: StateCounts::default(),
            sequences: Sequences::default(),
            refused: BTreeMap::new(),
            parked: BTreeSet::new(),
            unconfirmed_written: BTreeSet::new(),
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
        self.tips.appended(change.header());
        let author = change.header().author;
        self.counts_mut(author).unconfirmed += 1;
        let number = self.authors.number(author);
        let sequence = change.header().sequence;
        self.entries.push(Entry::new(change, number));
        let position = self.head();
        self.sequences.add(number, sequence, position);
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

    /// The change at a position, when the log holds it in memory: every
    /// change of a log that never released one, and every control change.
    /// [`Log::change_at`] also reads a released change from its store.
    pub fn get(&self, position: Position) -> Option<&Change> {
        let index = usize::try_from(position.0.checked_sub(1)?).ok()?;
        self.entries.get(index)?.held()
    }

    /// The change at a position: from memory, or read from `frames` when
    /// the log released it.
    pub fn change_at(&self, position: Position, frames: &mut impl Frames) -> Option<Held<'_>> {
        let index = usize::try_from(position.0.checked_sub(1)?).ok()?;
        let entry = self.entries.get(index)?;
        match entry.held() {
            Some(change) => Some(Cow::Borrowed(change)),
            None => frames.stored(self.space, entry.id).map(Cow::Owned),
        }
    }

    /// The change `id`: from memory, or read from `frames` when the log
    /// released it.
    pub fn change(&self, id: ChangeId, frames: &mut impl Frames) -> Option<Held<'_>> {
        self.change_at(self.position_of(id)?, frames)
    }

    /// The header of the change `id`, without its payload and without
    /// the keys this core does not know: what the log keeps of it in
    /// memory, whether it holds the change or released it (NOC-518).
    pub fn header(&self, id: ChangeId) -> Option<Header> {
        Some(self.entry(id)?.header(self.space, &self.authors))
    }

    /// Drops the bytes of the sealed change `id`, whose frame the log's
    /// store now keeps: the log keeps its header without the payload, and
    /// reads the change back through [`Frames`] when a pass needs it. A
    /// control change stays in memory. Says whether the log released it.
    pub fn release(&mut self, id: ChangeId) -> bool {
        let Some(entry) = self.entry_mut(id) else {
            return false;
        };
        if entry.kind == Kind::CONTROL {
            return false;
        }
        entry.held.take().is_some()
    }

    /// The ids of the log's changes, in log order.
    pub fn ids(&self) -> impl Iterator<Item = ChangeId> + '_ {
        self.entries.iter().map(|entry| entry.id)
    }

    /// The control changes of the log, in log order, each with its state.
    pub fn controls(&self) -> impl Iterator<Item = (&Change, Option<&State>)> {
        self.entries.iter().filter_map(|entry| {
            let change = entry.held()?;
            (entry.kind == Kind::CONTROL).then_some((change, entry.state.as_ref()))
        })
    }

    /// The member's state of a change, when the log holds it with one.
    pub fn state(&self, id: ChangeId) -> Option<&State> {
        self.entry(id)?.state.as_ref()
    }

    /// The relay's refusal of a change, when one was recorded.
    pub fn relay_refusal(&self, id: ChangeId) -> Option<&RelayRefusal> {
        self.entry(id)?.refusal.as_deref()
    }

    /// The changes of `author` the relay refused, in local order, with the
    /// relay's refusal of each.
    pub fn refused_by(
        &self,
        author: MemberId,
    ) -> impl Iterator<Item = (ChangeId, &RelayRefusal)> + '_ {
        self.refused
            .get(&author)
            .into_iter()
            .flatten()
            .filter_map(|position| {
                let entry = &self.entries[index(*position)];
                Some((entry.id, entry.refusal.as_deref()?))
            })
    }

    /// Records the relay's refusal of a change the log holds, beside the
    /// change; its state stays as it is (RFC 0002, "A change the relay refuses
    /// after the member folded it"). Returns `false`, and records
    /// nothing, when the log does not hold the change.
    pub fn record_relay_refusal(&mut self, id: ChangeId, refusal: RelayRefusal) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let entry = &mut self.entries[index];
        let newly = entry.refusal.replace(Box::new(refusal)).is_none();
        let (author, confirmed) = (self.authors.id(entry.author), entry.confirmed);
        if newly {
            let position = self.position_of(id).expect("the log holds the change");
            self.refused.entry(author).or_default().insert(position);
            let counts = self.counts_mut(author);
            counts.refused += 1;
            if !confirmed {
                counts.unconfirmed -= 1;
            }
        }
        true
    }

    /// Forgets the relay's refusal of a change the log holds, so that the
    /// member pushes it to the relay once more (RFC 0008, "How the relay applies
    /// the rules"); a change
    /// the relay has not confirmed counts as unconfirmed again. Returns
    /// `false`, and changes nothing, when the log does not hold the change
    /// or holds no refusal of it.
    pub fn clear_relay_refusal(&mut self, id: ChangeId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let entry = &mut self.entries[index];
        if entry.refusal.take().is_none() {
            return false;
        }
        let (author, confirmed) = (self.authors.id(entry.author), entry.confirmed);
        let position = self.position_of(id).expect("the log holds the change");
        if let Some(positions) = self.refused.get_mut(&author) {
            positions.remove(&position);
        }
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
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let entry = &mut self.entries[index];
        let newly = !std::mem::replace(&mut entry.confirmed, true);
        let (author, refused) = (self.authors.id(entry.author), entry.refusal.is_some());
        let written_at = entry.written_at;
        if newly && !refused {
            self.counts_mut(author).unconfirmed -= 1;
        }
        if let Some(at) = written_at {
            let position = self.position_of(id).expect("the log holds the change");
            self.unconfirmed_written.remove(&(at, position));
        }
        true
    }

    /// Records when the member wrote a change the log holds, in
    /// milliseconds since the Unix epoch. Returns `false`, and records
    /// nothing, when the log does not hold the change.
    pub fn record_written_at(&mut self, id: ChangeId, at: u64) -> bool {
        let Some(position) = self.position_of(id) else {
            return false;
        };
        let entry = &mut self.entries[index(position)];
        if let Some(old) = entry.written_at.replace(at) {
            self.unconfirmed_written.remove(&(old, position));
        }
        if !entry.confirmed {
            self.unconfirmed_written.insert((at, position));
        }
        true
    }

    /// When the member wrote the oldest of its changes the relay has not
    /// confirmed, in milliseconds since the Unix epoch; `None` when every
    /// change it wrote, or recorded a time for, is confirmed.
    pub fn oldest_unconfirmed_written(&self) -> Option<u64> {
        self.unconfirmed_written.first().map(|(at, _)| *at)
    }

    /// Forgets every confirmation of the relay, whose epoch changed;
    /// refusals stay (RFC 0007, "Restores and the relay epoch").
    pub fn forget_relay_confirmations(&mut self) {
        let mut returned = Vec::new();
        for (entry, position) in self.entries.iter_mut().zip(1..) {
            if !std::mem::take(&mut entry.confirmed) {
                continue;
            }
            if let Some(at) = entry.written_at {
                self.unconfirmed_written.insert((at, Position(position)));
            }
            if entry.refusal.is_none() {
                returned.push(entry.author);
            }
        }
        for author in returned {
            let author = self.authors.id(author);
            self.counts_mut(author).unconfirmed += 1;
        }
    }

    /// The newest model version the parked data of the member's own model
    /// needs (RFC 0002, "What the model version is for"): the highest
    /// version in payload key 64 among the changes parked for an unknown
    /// "must understand" payload key and the folded changes with parked
    /// operations, whose key 64 names `model`. A change parked as another
    /// model, or for a payload this core cannot read, does not count, nor
    /// does one without key 64. It opens each such change with `checks`,
    /// so it costs one decryption per counted change, and reads it from
    /// `frames` when the log released it.
    pub fn newest_model(
        &self,
        checks: &impl Checks,
        model: [u8; 16],
        frames: &mut impl Frames,
    ) -> Option<u64> {
        self.parked
            .iter()
            .filter_map(|position| {
                let change = self.change_at(*position, frames)?;
                let writer = checks.open(&change).ok()?.writer_model()?.ok()?;
                (writer.id == model).then_some(writer.version)
            })
            .max()
    }

    /// How many changes are parked and rejected, and how many operations
    /// of folded changes are parked, per reason.
    pub fn state_counts(&self) -> &StateCounts {
        &self.state_counts
    }

    /// The changes of every equivocation in this log, in local order: two
    /// or more different changes with the same author and sequence (RFC
    /// 0002, "Author sequence"). Both are kept and folded; the SDK reports
    /// them.
    pub fn equivocations(&self) -> Vec<ChangeId> {
        let mut positions: Vec<Position> = self.sequences.equivocating().collect();
        positions.sort();
        positions
            .into_iter()
            .map(|position| self.entries[index(position)].id)
            .collect()
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
        if let Some(old) = &entry.state {
            self.state_counts.count(old, false);
        }
        self.state_counts.count(&state, true);
        let position = Position(index as u64 + 1);
        if has_parked_data(&state) {
            self.parked.insert(position);
        } else {
            self.parked.remove(&position);
        }
        entry.state = Some(state);
        if folded(&entry.state) {
            match entry.held().filter(|_| entry.kind == Kind::CONTROL) {
                Some(control) => self.tips.folded_control(entry.id, control.header()),
                None => self.tips.folded_sealed(entry.id, opened),
            }
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
    /// once, here, read from `frames` when the log released it; one that
    /// does not open is left out until it does.
    pub fn tips(&mut self, checks: &impl Checks, frames: &mut impl Frames) -> &Tips {
        for id in self.tips.unread().clone() {
            let Some(change) = self.change(id, frames) else {
                continue;
            };
            if let Ok(payload) = checks.open(&change) {
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
    pub(crate) fn rejected(&self) -> Vec<ChangeId> {
        self.entries
            .iter()
            .filter(|entry| matches!(entry.state, Some(State::Rejected(_))))
            .map(|entry| entry.id)
            .collect()
    }

    /// The control changes that passed their checks, in log order.
    pub(crate) fn folded_controls(&self) -> impl Iterator<Item = &Change> {
        self.controls()
            .filter(|(_, state)| matches!(state, Some(State::Folded { .. })))
            .map(|(change, _)| change)
    }

    /// The folded changes that are not control changes, in log order, with
    /// their parked operations. [`Log::change`] gives each change.
    pub(crate) fn folded_sealed(&self) -> Vec<(ChangeId, Vec<ParkedOperation>)> {
        self.entries
            .iter()
            .filter(|entry| entry.kind != Kind::CONTROL)
            .filter_map(|entry| match &entry.state {
                Some(State::Folded { parked }) => Some((entry.id, parked.clone())),
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

    /// The entries after a cursor, in order, with their positions: each
    /// from memory, or read from `frames` when the log released it. An
    /// entry `frames` cannot read is left out.
    pub fn after<'a, F: Frames>(
        &'a self,
        cursor: Position,
        frames: &'a mut F,
    ) -> impl Iterator<Item = (Position, Held<'a>)> + 'a {
        let skip = usize::try_from(cursor.0).unwrap_or(usize::MAX);
        let space = self.space;
        self.entries
            .iter()
            .zip(1..)
            .skip(skip)
            .filter_map(move |(entry, position)| {
                let change = match entry.held() {
                    Some(change) => Cow::Borrowed(change),
                    None => Cow::Owned(frames.stored(space, entry.id)?),
                };
                Some((Position(position), change))
            })
    }
}

/// The index in a log's entries of the entry at `position`, which the log
/// holds.
fn index(position: Position) -> usize {
    usize::try_from(position.0 - 1).expect("a position the log holds fits its entries")
}

/// Whether a change in `state` holds parked data whose model
/// [`Log::newest_model`] reads: parked for an unknown "must understand"
/// payload key, or folded with parked operations.
fn has_parked_data(state: &State) -> bool {
    match state {
        State::Parked(ParkReason::UnknownPayloadKey) => true,
        State::Folded { parked } => !parked.is_empty(),
        _ => false,
    }
}
