//! Where each rich-text operation went (RFC 0013, "Outcomes: applied,
//! replaced and lost"), computed from the state alone:
//!
//! - **lost**, when an id it added is deleted in the document although no
//!   operation's delete set names it: Yjs deleted it because a concurrent
//!   change deleted the element it sits in, or overrode the attribute it
//!   set;
//! - **replaced**, when an id it added is deleted and every such id is
//!   named by a delete set: the earliest operation in stamp order whose
//!   delete set names one of them replaced it;
//! - **applied** (visible) otherwise, placed or pending.

use std::collections::BTreeSet;

use super::state::State;
use super::update::{self, Range};
use crate::field::Outcome;
use crate::ids::OperationId;

/// The outcome of the operation `id`, or `None` when the state never took
/// it.
pub(super) fn of(state: &State, id: OperationId) -> Option<Outcome> {
    let record = state.edits.get(&id)?;
    let gone = intersect(
        &deleted(state),
        &merge(record.runs.iter().map(|run| run.range).collect()),
    );
    if gone.is_empty() {
        return Some(Outcome::Visible);
    }
    if !covered(&gone, &named(state)) {
        return Some(Outcome::Lost);
    }
    let by = state
        .edits
        .iter()
        .filter(|(_, other)| !intersect(&gone, &merge(other.deletes.clone())).is_empty())
        .min_by_key(|(other_id, other)| other.order(**other_id))
        .map(|(other_id, _)| *other_id)
        .expect("a deleted and named id is named by some operation");
    Some(Outcome::Replaced(by))
}

/// The operations whose outcome is lost.
pub(super) fn lost(state: &State) -> BTreeSet<OperationId> {
    state
        .edits
        .keys()
        .filter(|id| of(state, **id) == Some(Outcome::Lost))
        .copied()
        .collect()
}

/// The ids the document holds as deleted: its state's delete set, which
/// covers the items garbage collection dropped.
fn deleted(state: &State) -> Vec<Range> {
    match state.document.is_empty() {
        true => Vec::new(),
        false => update::read(&state.document)
            .map(|shape| shape.deletes)
            .unwrap_or_default(),
    }
}

/// The ids some operation's delete set names.
fn named(state: &State) -> Vec<Range> {
    merge(
        state
            .edits
            .values()
            .flat_map(|record| record.deletes.iter().copied())
            .collect(),
    )
}

/// Ranges sorted by client and clock, overlapping and adjacent ones joined.
fn merge(mut ranges: Vec<Range>) -> Vec<Range> {
    ranges.sort();
    let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
    for range in ranges {
        match merged.last_mut() {
            Some(last) if last.client == range.client && range.clock <= last.end() => {
                last.len = last.len.max(range.end() - last.clock);
            }
            _ => merged.push(range),
        }
    }
    merged
}

/// The ids in both of two merged lists.
fn intersect(a: &[Range], b: &[Range]) -> Vec<Range> {
    let mut both = Vec::new();
    for x in a {
        for y in b.iter().filter(|y| y.client == x.client) {
            let clock = x.clock.max(y.clock);
            let end = x.end().min(y.end());
            if clock < end {
                both.push(Range {
                    client: x.client,
                    clock,
                    len: end - clock,
                });
            }
        }
    }
    both
}

/// Whether every id of `ranges` is in the merged list `by`.
fn covered(ranges: &[Range], by: &[Range]) -> bool {
    ranges.iter().all(|range| {
        by.iter()
            .any(|b| b.client == range.client && b.clock <= range.clock && range.end() <= b.end())
    })
}
