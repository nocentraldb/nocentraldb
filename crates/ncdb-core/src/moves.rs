//! Later edits (RFC 0016, "Later edits"): an operation on a moved object,
//! in the old space, that is not in the causal past of the move that won
//! ("Two moves at once"). Every member of the old space computes them from
//! the old space's folded changes alone, as it computes a concurrent loser
//! (RFC 0002, "A late change stays identifiable"): the causal past of a
//! change is what its dependencies reach, so every member finds the same
//! set in any order of arrival. A later edit is kept and folded onto the
//! trace; it never reaches the new space by itself, and the app carries it
//! over.
//!
//! A losing concurrent move is not a later edit: it is a conflict of the
//! move, which [`Object::moving`] reads. An operation in the winning move's
//! own change belongs to its past, as the mover wrote it with the move.

use std::collections::{BTreeMap, BTreeSet};

use crate::change::Operation;
use crate::clock::Hlc;
use crate::field_op::{self, MOVE};
use crate::ids::{ChangeId, MemberId, ObjectId, OperationId};
use crate::log::Log;
use crate::object::{MOVED_FROM, Object};
use crate::receive::Checks;

/// One later edit: a folded operation of the old space on a moved object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaterEdit {
    /// The operation's id.
    pub id: OperationId,
    /// The member that signed its change.
    pub author: MemberId,
    /// Its change's clock.
    pub stamp: Hlc,
    /// The operation, as its change holds it.
    pub operation: Operation,
}

impl LaterEdit {
    /// What the operation takes from its change: its id, author and stamp.
    pub fn origin(&self) -> field_op::Origin {
        field_op::Origin {
            id: self.id,
            author: self.author,
            stamp: self.stamp,
        }
    }
}

/// The later edits of each moved object of `log`'s space, in clock order
/// (stamp, then author, then id). `checks` opens the folded changes; a
/// change that no longer opens counts for nothing. An object that is not
/// moved, or whose move was taken back, has none.
pub fn later_edits(log: &Log, checks: &impl Checks) -> BTreeMap<ObjectId, Vec<LaterEdit>> {
    let mut dependencies: BTreeMap<ChangeId, BTreeSet<ChangeId>> = BTreeMap::new();
    let mut folded = Vec::new();
    for (change, parked) in log.folded_sealed() {
        let Ok(payload) = checks.open_folded(&change) else {
            continue;
        };
        let parked: BTreeSet<u64> = parked.iter().map(|p| p.position).collect();
        for (position, operation) in (0..).zip(payload.operations) {
            if !parked.contains(&position) {
                folded.push(LaterEdit {
                    id: change.operation_id(position),
                    author: change.header().author,
                    stamp: payload.clock,
                    operation,
                });
            }
        }
        dependencies.insert(change.id(), payload.dependencies);
    }
    let objects = objects_of(&folded);
    let mut later: BTreeMap<ObjectId, Vec<LaterEdit>> = BTreeMap::new();
    for (object, state) in &objects {
        if state.moved_to().is_none() {
            continue;
        }
        let winner = state.moving().value.expect("a moved object has a move").id;
        let past = causal_past(winner.change, &dependencies);
        let mut edits: Vec<LaterEdit> = folded
            .iter()
            .filter(|e| e.operation.object == *object && !past.contains(&e.id.change))
            .filter(|e| !is_move(&e.operation))
            .cloned()
            .collect();
        edits.sort_by_key(|e| (e.stamp, e.author, e.id));
        if !edits.is_empty() {
            later.insert(*object, edits);
        }
    }
    later
}

/// Each object of `log`'s space that any folded move names, with what the
/// copy holds of it: its creates, its deletion and its move, whose
/// conflicts are the moves that lost (RFC 0016, "Two moves at once"). An
/// object whose move was taken back is among them, and reads as not moved.
pub fn moved_objects(log: &Log, checks: &impl Checks) -> BTreeMap<ObjectId, Object> {
    let mut folded = Vec::new();
    for (change, parked) in log.folded_sealed() {
        let Ok(payload) = checks.open_folded(&change) else {
            continue;
        };
        let parked: BTreeSet<u64> = parked.iter().map(|p| p.position).collect();
        for (position, operation) in (0..).zip(payload.operations) {
            if !parked.contains(&position) && operation.field.is_none() {
                folded.push(LaterEdit {
                    id: change.operation_id(position),
                    author: change.header().author,
                    stamp: payload.clock,
                    operation,
                });
            }
        }
    }
    let moved: BTreeSet<ObjectId> = folded
        .iter()
        .filter(|e| e.operation.op == MOVE)
        .map(|e| e.operation.object)
        .collect();
    let mut objects = objects_of(&folded);
    objects.retain(|object, _| moved.contains(object));
    objects
}

/// The objects the object operations among `folded` make.
fn objects_of(folded: &[LaterEdit]) -> BTreeMap<ObjectId, Object> {
    let mut objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    for edit in folded.iter().filter(|e| e.operation.field.is_none()) {
        let origin = field_op::Origin {
            id: edit.id,
            author: edit.author,
            stamp: edit.stamp,
        };
        if let Ok(operation) = field_op::decode_object(&edit.operation, origin) {
            objects
                .entry(edit.operation.object)
                .or_default()
                .apply(&operation);
        }
    }
    objects
}

/// Whether `operation` belongs to the move itself: a move, or a set of
/// `$movedFrom`.
fn is_move(operation: &Operation) -> bool {
    match operation.field.as_deref() {
        None => operation.op == MOVE,
        Some(field) => field == MOVED_FROM,
    }
}

/// `change` and every change its dependencies reach, among those held.
fn causal_past(
    change: ChangeId,
    dependencies: &BTreeMap<ChangeId, BTreeSet<ChangeId>>,
) -> BTreeSet<ChangeId> {
    let mut past = BTreeSet::from([change]);
    let mut next = vec![change];
    while let Some(id) = next.pop() {
        for dependency in dependencies.get(&id).into_iter().flatten() {
            if past.insert(*dependency) {
                next.push(*dependency);
            }
        }
    }
    past
}
