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

use crate::change::{Change, Kind};
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
/// the set depends on.
#[derive(Clone, Debug, Default)]
struct Heads {
    /// The changes of the set, each with its dependencies.
    members: BTreeMap<ChangeId, BTreeSet<ChangeId>>,
    /// How many changes of the set depend on each change.
    covered: BTreeMap<ChangeId, usize>,
    heads: BTreeSet<ChangeId>,
}

impl Heads {
    fn insert(&mut self, id: ChangeId, dependencies: BTreeSet<ChangeId>) {
        self.remove(id);
        for dependency in &dependencies {
            *self.covered.entry(*dependency).or_default() += 1;
            self.heads.remove(dependency);
        }
        if !self.covered.contains_key(&id) {
            self.heads.insert(id);
        }
        self.members.insert(id, dependencies);
    }

    fn remove(&mut self, id: ChangeId) {
        let Some(dependencies) = self.members.remove(&id) else {
            return;
        };
        self.heads.remove(&id);
        for dependency in dependencies {
            let Some(count) = self.covered.get_mut(&dependency) else {
                continue;
            };
            *count -= 1;
            if *count == 0 {
                self.covered.remove(&dependency);
                if self.members.contains_key(&dependency) {
                    self.heads.insert(dependency);
                }
            }
        }
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
    /// The clock of each folded sealed change that was opened.
    clocks: BTreeMap<ChangeId, Hlc>,
    /// How many of those changes carry each clock, so the highest is known
    /// when one leaves.
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
    pub(crate) fn appended(&mut self, change: &Change) {
        let header = change.header();
        let highest = self.sequences.entry(header.author).or_default();
        *highest = (*highest).max(header.sequence);
    }

    /// A change of the log was folded. A sealed change's dependencies and
    /// clock come from `opened`, when the checks opened it; otherwise it
    /// waits in `unread` until [`Tips::read`].
    pub(crate) fn folded(&mut self, change: &Change, opened: Option<Opened>) {
        let id = change.id();
        let header = change.header();
        if header.kind == Kind::CONTROL {
            let dependencies = ControlBody::decode(&header.payload)
                .map(|body| body.dependencies)
                .unwrap_or_default();
            self.controls.insert(id, dependencies.clone());
            self.all.insert(id, dependencies);
            return;
        }
        match opened {
            Some(opened) => self.read(id, opened),
            None => {
                self.unread.insert(id);
            }
        }
    }

    /// A folded change of the log is folded no more.
    pub(crate) fn unfolded(&mut self, id: ChangeId) {
        self.all.remove(id);
        self.controls.remove(id);
        self.unread.remove(&id);
        self.forget_clock(id);
    }

    /// The folded sealed changes whose payload was not read yet.
    pub(crate) fn unread(&self) -> &BTreeSet<ChangeId> {
        &self.unread
    }

    /// What the payload of the folded sealed change `id` gave.
    pub(crate) fn read(&mut self, id: ChangeId, opened: Opened) {
        self.unread.remove(&id);
        self.all.insert(id, opened.dependencies);
        self.forget_clock(id);
        self.clocks.insert(id, opened.clock);
        *self.seen.entry(opened.clock).or_default() += 1;
    }

    /// Takes the clock of `id` out of the clocks seen, when it is there.
    fn forget_clock(&mut self, id: ChangeId) {
        let Some(clock) = self.clocks.remove(&id) else {
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
