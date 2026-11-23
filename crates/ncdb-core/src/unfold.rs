//! Un-folding (RFC 0006, "Un-folding"; RFC 0008, "What a new change can
//! change"): a control change that arrives late can make a sealed change
//! the member already folded fail its membership check, for example a
//! revocation whose cutoff lies below the change, or its decryption, when
//! a key chain entry with a lower change id replaces the key of its
//! generation (NOC-183). The change then moves from folded to rejected,
//! and the fold folds every field and object it touched again from the
//! operations of the changes still folded. A field's state is a function
//! of its set of operations (RFC 0004), so the fold then equals the fold
//! of a member that never folded the change.
//!
//! Steps 4 and 5 are checked again, step 5 through
//! [`Checks::still_opens`]. The authors' keys the checks hold only grow as
//! control changes arrive (RFC 0002, "Rejected changes"; NOC-78), so a
//! change that passed step 3
//! still passes it. Control changes are never un-folded: their verdicts are
//! `resolve`'s, recomputed over every control change held.
//!
//! The changes still folded are opened again and scanned for the fields
//! they touch. RFC 0006 allows a bookkeeping table from object and field to
//! operations, to make this a lookup; un-folding is rare, and the scan
//! needs no table that could disagree with the log.

use std::collections::{BTreeMap, BTreeSet};

use crate::change::{Change, Operation};
use crate::field_op::Origin;
use crate::ids::ChangeId;
use crate::ids::ObjectId;
use crate::log::{Log, OperationParkReason, ParkedOperation, RejectReason, State};
use crate::receive::{Checks, Fold, fold_one, understand};
use crate::tips::Opened;

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
/// control state no longer admits, or that no longer opens, and folds what
/// they touched again from the changes still folded.
pub(crate) fn unfold(log: &mut Log, checks: &impl Checks, fold: &mut impl Fold) {
    let failing: Vec<_> = log
        .folded_sealed()
        .into_iter()
        .filter_map(|(change, parked)| {
            let state = if !checks.is_member(&change) {
                State::Rejected(RejectReason::NotMember)
            } else {
                checks.still_opens(&change).err()?.into()
            };
            Some((change, parked, state))
        })
        .collect();
    if failing.is_empty() {
        return;
    }
    let mut touched = BTreeSet::new();
    for (change, parked, state) in &failing {
        log.set_state(change.id(), state.clone());
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

/// Folds again each field of the objects whose move was taken back
/// ([`Fold::take_revived`]): the fold kept none of their fields while
/// they were traces (RFC 0016). Each field is folded again in stamp order,
/// as [`refold_displaced`] does, so that an operation that parks for an
/// earlier one, as a rich-text operation can, is recorded as parked on
/// every copy, whatever arrived while the object was a trace.
pub(crate) fn fold_revived(log: &mut Log, checks: &impl Checks, fold: &mut impl Fold) {
    let revived = fold.take_revived();
    if revived.is_empty() {
        return;
    }
    let mut fields = BTreeSet::new();
    for (change, _) in log.folded_sealed() {
        let Ok(payload) = checks.open_folded(&change) else {
            continue;
        };
        for operation in payload.operations {
            if operation.field.is_some() && revived.contains(&operation.object) {
                fields.insert(Target::of(&operation));
            }
        }
    }
    for target in &fields {
        refold(log, checks, fold, target);
    }
}

/// Folds again, in stamp order, every field an operation displaced others
/// in (RFC 0013, "The same id twice"): the field is forgotten, and every
/// operation on it of a folded change, also one parked as "malformed
/// arguments", folds again from the earliest in stamp order (RFC 0004,
/// "Tie-break"). An operation that parks now is recorded as parked, and
/// one that folds now no longer is. Folded in that order, an operation
/// only ever parks for an earlier one, so the field and the states of its
/// changes are those of every order of arrival.
pub(crate) fn refold_displaced(log: &mut Log, checks: &impl Checks, fold: &mut impl Fold) {
    loop {
        let targets = fold.take_refolds();
        if targets.is_empty() {
            return;
        }
        for (object, collection, field) in targets {
            refold(log, checks, fold, &Target::Field(object, collection, field));
        }
    }
}

fn refold(log: &mut Log, checks: &impl Checks, fold: &mut impl Fold, target: &Target) {
    let Target::Field(object, collection, field) = target else {
        return;
    };
    let mut operations = Vec::new();
    let mut parked_before = BTreeMap::new();
    for (change, parked) in log.folded_sealed() {
        let Ok(payload) = checks.open_folded(&change) else {
            continue;
        };
        let opened = Opened {
            dependencies: payload.dependencies.clone(),
            clock: payload.clock,
        };
        for (position, operation) in (0..).zip(payload.operations) {
            if Target::of(&operation) != *target {
                continue;
            }
            let reason = parked
                .iter()
                .find(|p| p.position == position)
                .map(|p| p.reason);
            if reason.is_some_and(|r| r != OperationParkReason::MalformedArguments) {
                continue;
            }
            let origin = Origin {
                id: change.operation_id(position),
                author: change.header().author,
                stamp: payload.clock,
            };
            operations.push((origin, operation));
            parked_before.insert(change.id(), (parked.clone(), opened.clone()));
        }
    }
    operations.sort_by_key(|(origin, _)| (origin.stamp, origin.author, origin.id));
    fold.refolding(Some((*object, collection, field)));
    fold.clear_field(*object, collection, field);
    let mut parked_now: BTreeMap<ChangeId, BTreeMap<u64, OperationParkReason>> = BTreeMap::new();
    for (origin, operation) in &operations {
        let result = understand(operation, fold).and_then(|()| fold_one(operation, *origin, fold));
        let positions = parked_now.entry(origin.id.change).or_default();
        if let Err(reason) = result {
            positions.insert(origin.id.position, reason);
        }
    }
    fold.refolding(None);
    let ours: BTreeSet<(ChangeId, u64)> = operations
        .iter()
        .map(|(origin, _)| (origin.id.change, origin.id.position))
        .collect();
    for (change, (before, opened)) in parked_before {
        let mut parked: BTreeMap<u64, OperationParkReason> = before
            .into_iter()
            .filter(|p| !ours.contains(&(change, p.position)))
            .map(|p| (p.position, p.reason))
            .collect();
        parked.extend(parked_now.remove(&change).unwrap_or_default());
        let parked: Vec<ParkedOperation> = parked
            .into_iter()
            .map(|(position, reason)| ParkedOperation { position, reason })
            .collect();
        if log.state(change)
            != Some(&State::Folded {
                parked: parked.clone(),
            })
        {
            log.set_state_opened(change, State::Folded { parked }, Some(opened));
        }
    }
}

/// The operations of a folded change that were folded, not parked, each
/// with its origin. The change opened when it was folded, and
/// [`Checks::open_folded`] opens it again with that key.
fn folded_operations(
    change: &Change,
    parked: &[ParkedOperation],
    checks: &impl Checks,
) -> Vec<(Origin, Operation)> {
    let Ok(payload) = checks.open_folded(change) else {
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
