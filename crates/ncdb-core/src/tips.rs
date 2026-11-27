//! The tips of a space's log (RFC 0002, "The change" and "Dependencies"):
//! what a member's next change in the space takes from the log besides
//! its generation. Those are the heads it depends on, the author sequence
//! it carries and the highest clock its clock observes. A control change
//! names its dependencies among the control changes alone (RFC 0008), so
//! the heads among the folded control changes are kept too.
//!
//! The [`Log`](crate::log::Log) keeps them as it appends changes and sets
//! their states: a change folded, received or checked again, adds to them
//! and one un-folded leaves them, so reading them costs no pass over the
//! log. Only the dependencies and the clock of a sealed change need its
//! payload. A change folded through [`Log::receive`](crate::log::Log) was
//! opened by the checks then, and the log keeps what it read. A change
//! whose state was read back from storage, as when a view is reopened or
//! folded again, is opened once, the first time the tips are read
//! ([`Log::tips`](crate::log::Log::tips)).
//!
//! The tips equal what a full recount of the log gives: the heads are the
//! folded changes no other folded change depends on, a change the log
//! holds but did not fold being no head, since a rejected or parked
//! sealed change cannot be read; the next sequence of an author is one
//! more than the highest of its changes the log holds, in any state; and
//! the clock is the highest among the folded sealed changes.

use std::collections::{BTreeMap, BTreeSet};

use crate::change::Header;
use crate::clock::Hlc;
use crate::ids::{ChangeId, MemberId};
use crate::membership::ControlBody;

/// What a sealed change's payload gave the tips when it was opened.
#[derive(Clone, Debug)]
pub(crate) struct Opened {
    pub(crate) dependencies: BTreeSet<ChangeId>,
    pub(crate) clock: Hlc,
}

/// The heads among a set of changes: the changes of the set no change of
/// the set depends on. One node per change of the set and per change one
/// of them depends on keeps what the heads need, so a change costs one
/// map entry and its dependencies as a slice (NOC-518).
#[derive(Clone, Debug, Default)]
struct Heads {
    nodes: BTreeMap<ChangeId, Node>,
    heads: BTreeSet<ChangeId>,
}

/// What [`Heads`] keeps of one change, in 32 bytes.
#[derive(Clone, Debug, Default)]
struct Node {
    /// The change's dependencies, sorted and without repeats, when the
    /// change is in the set.
    dependencies: Option<Box<[ChangeId]>>,
    /// The change's clock, when `clocked`.
    clock: Hlc,
    /// How many changes of the set depend on it.
    covered: u32,
    /// Whether the change has a clock: a sealed change in the set whose
    /// payload was read.
    clocked: bool,
}

impl Node {
    /// Takes the change's clock, if it has one.
    fn take_clock(&mut self) -> Option<Hlc> {
        std::mem::take(&mut self.clocked).then_some(self.clock)
    }
}

impl Heads {
    /// Adds `id` to the set, with its dependencies and its clock, and
    /// returns the clock it had in the set before, if any.
    fn insert(
        &mut self,
        id: ChangeId,
        dependencies: BTreeSet<ChangeId>,
        clock: Option<Hlc>,
    ) -> Option<Hlc> {
        let old = self.remove(id);
        for dependency in &dependencies {
            self.nodes.entry(*dependency).or_default().covered += 1;
            self.heads.remove(dependency);
        }
        let node = self.nodes.entry(id).or_default();
        if node.covered == 0 {
            self.heads.insert(id);
        }
        node.dependencies = Some(dependencies.into_iter().collect());
        node.clocked = clock.is_some();
        node.clock = clock.unwrap_or_default();
        old
    }

    /// Takes `id` out of the set, and returns the clock it had in it, if
    /// any.
    fn remove(&mut self, id: ChangeId) -> Option<Hlc> {
        let node = self.nodes.get_mut(&id)?;
        let dependencies = node.dependencies.take()?;
        let clock = node.take_clock();
        if node.covered == 0 {
            self.nodes.remove(&id);
        }
        self.heads.remove(&id);
        for dependency in dependencies {
            let Some(node) = self.nodes.get_mut(&dependency) else {
                continue;
            };
            node.covered -= 1;
            if node.covered == 0 {
                if node.dependencies.is_some() {
                    self.heads.insert(dependency);
                } else {
                    self.nodes.remove(&dependency);
                }
            }
        }
        clock
    }
}

/// The tips of a space's log: its heads, the heads among its control
/// changes, each author's next sequence and the highest clock.
#[derive(Clone, Debug, Default)]
pub struct Tips {
    /// The highest sequence of each author among the changes the log holds.
    sequences: BTreeMap<MemberId, u64>,
    /// The folded changes whose dependencies are known.
    all: Heads,
    /// The folded control changes.
    controls: Heads,
    /// How many folded sealed changes that were opened carry each clock,
    /// so the highest is known when one leaves; `all` keeps the clock of
    /// each.
    seen: BTreeMap<Hlc, usize>,
    /// The folded sealed changes whose payload was not read yet.
    unread: BTreeSet<ChangeId>,
}

impl Tips {
    /// The heads among the folded changes, which the member's next change
    /// depends on.
    pub fn heads(&self) -> &BTreeSet<ChangeId> {
        &self.all.heads
    }

    /// The heads among the folded control changes, which the member's next
    /// control change depends on (RFC 0008).
    pub fn control_heads(&self) -> &BTreeSet<ChangeId> {
        &self.controls.heads
    }

    /// The next author sequence of `author`: one more than the highest of
    /// its changes the log holds, in any state.
    pub fn next_sequence(&self, author: MemberId) -> u64 {
        self.sequences
            .get(&author)
            .map_or(1, |sequence| sequence + 1)
    }

    /// The highest clock among the folded sealed changes, which the
    /// member's clock observes before its next change.
    pub fn seen(&self) -> Hlc {
        self.seen.keys().next_back().copied().unwrap_or_default()
    }

    /// A change was appended to the log.
    pub(crate) fn appended(&mut self, header: &Header) {
        let highest = self.sequences.entry(header.author).or_default();
        *highest = (*highest).max(header.sequence);
    }

    /// A control change of the log was folded. Its header carries its
    /// payload, its control body, which names its dependencies.
    pub(crate) fn folded_control(&mut self, id: ChangeId, header: &Header) {
        let dependencies = ControlBody::decode(&header.payload)
            .map(|body| body.dependencies)
            .unwrap_or_default();
        self.controls.insert(id, dependencies.clone(), None);
        let clock = self.all.insert(id, dependencies, None);
        self.forget_clock(clock);
    }

    /// A sealed change of the log was folded. Its dependencies and clock
    /// come from `opened`, when the checks opened it; otherwise it waits
    /// in `unread` until [`Tips::read`].
    pub(crate) fn folded_sealed(&mut self, id: ChangeId, opened: Option<Opened>) {
        match opened {
            Some(opened) => self.read(id, opened),
            None => {
                self.unread.insert(id);
            }
        }
    }

    /// A folded change of the log is folded no more.
    pub(crate) fn unfolded(&mut self, id: ChangeId) {
        let clock = self.all.remove(id);
        self.forget_clock(clock);
        self.controls.remove(id);
        self.unread.remove(&id);
    }

    /// The folded sealed changes whose payload was not read yet.
    pub(crate) fn unread(&self) -> &BTreeSet<ChangeId> {
        &self.unread
    }

    /// What the payload of the folded sealed change `id` gave.
    pub(crate) fn read(&mut self, id: ChangeId, opened: Opened) {
        self.unread.remove(&id);
        let old = self.all.insert(id, opened.dependencies, Some(opened.clock));
        self.forget_clock(old);
        *self.seen.entry(opened.clock).or_default() += 1;
    }

    /// Takes a clock a change no longer carries out of the clocks seen.
    fn forget_clock(&mut self, clock: Option<Hlc>) {
        let Some(clock) = clock else {
            return;
        };
        if let Some(count) = self.seen.get_mut(&clock) {
            *count -= 1;
            if *count == 0 {
                self.seen.remove(&clock);
            }
        }
    }
}
