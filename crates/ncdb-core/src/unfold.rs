//! Un-folding (RFC 0006, "Un-folding"; RFC 0008, "What a new change can
//! change"): a control change that arrives late can make a sealed change
//! the member already folded fail its membership check, for example a
//! revocation whose cutoff lies below the change. The change then moves
//! from folded to rejected, and the fold folds every field and object it
//! touched again from the operations of the changes still folded. A
//! field's state is a function of its set of operations (RFC 0004), so the
//! fold then equals the fold of a member that never folded the change.
//!
//! Only step 4 is checked again. The authors' keys and the space keys the
//! checks hold only grow as control changes arrive (D11; NOC-78), so a
//! change that passed steps 3 and 5 still passes them. Control changes are
//! never un-folded: their verdicts are `resolve`'s, recomputed over every
//! control change held.
//!
//! The changes still folded are opened again and scanned for the fields
//! they touch. RFC 0006 allows a bookkeeping table from object and field to
//! operations, to make this a lookup; un-folding is rare, and the scan
//! needs no table that could disagree with the log.

use std::collections::BTreeSet;

use crate::change::{Change, Operation};
use crate::field_op::Origin;
use crate::ids::ObjectId;
use crate::log::{Log, ParkedOperation, RejectReason, State};
use crate::receive::{Checks, Fold, fold_one, understand};

/// What an operation folds into: a field of an object, or the object's
/// creates and deletes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Object(ObjectId),
    Field(ObjectId, String, String),
}

impl Target {
    fn of(operation: &Operation) -> Target {
        match &operation.field {
            Some(field) => Target::Field(
                operation.object,
                operation.collection.clone(),
                field.clone(),
            ),
            None => Target::Object(operation.object),
        }
    }
}

/// Un-folds every folded sealed change of the log whose author the
/// control state no longer admits, and folds what they touched again from
/// the changes still folded.
pub(crate) fn unfold(log: &mut Log, checks: &impl Checks, fold: &mut impl Fold) {
    let failing: Vec<_> = log
        .folded_sealed()
        .into_iter()
        .filter(|(change, _)| !checks.is_member(change))
        .collect();
    if failing.is_empty() {
        return;
    }
    let mut touched = BTreeSet::new();
    for (change, parked) in &failing {
        log.set_state(change.id(), State::Rejected(RejectReason::NotMember));
        touched.extend(
            folded_operations(change, parked, checks)
                .iter()
                .map(|(_, operation)| Target::of(operation)),
        );
    }
    for target in &touched {
        match target {
            Target::Field(object, collection, field) => {
                fold.clear_field(*object, collection, field);
            }
            Target::Object(object) => fold.clear_object(*object),
        }
    }
    for (change, parked) in log.folded_sealed() {
        for (origin, operation) in folded_operations(&change, &parked, checks) {
            if touched.contains(&Target::of(&operation)) {
                // It folded before and the model has not changed, so it
                // folds again.
                let _ =
                    understand(&operation, fold).and_then(|()| fold_one(&operation, origin, fold));
            }
        }
    }
}

/// The operations of a folded change that were folded, not parked, each
/// with its origin. The change opened when it was folded, and the keys the
/// checks hold only grow, so it opens again.
fn folded_operations(
    change: &Change,
    parked: &[ParkedOperation],
    checks: &impl Checks,
) -> Vec<(Origin, Operation)> {
    let Ok(payload) = checks.open(change) else {
        return Vec::new();
    };
    let parked: BTreeSet<u64> = parked.iter().map(|p| p.position).collect();
    (0..)
        .zip(payload.operations)
        .filter(|(position, _)| !parked.contains(position))
        .map(|(position, operation)| {
            let origin = Origin {
                id: change.operation_id(position),
                author: change.header().author,
                stamp: payload.clock,
            };
            (origin, operation)
        })
        .collect()
}
