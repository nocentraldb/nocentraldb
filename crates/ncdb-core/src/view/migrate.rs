//! An additive change of the model migrates the view in place (RFC 0006,
//! "Rebuilding the view"; RFC 0009, "What `nc.open` does", step 4;
//! NOC-156): a new declared index is a `CREATE INDEX`, a new collection
//! gets its tables and a new field its column, and then only the
//! operations parked as an unknown collection or field are folded, in the
//! transaction that opens the view. No other change is opened.
//!
//! It runs only when the view's tables were folded by the model of the
//! stored lock and the new model keeps every collection, field and index
//! of it as it was. Anything else, an app rolled back to an older lock
//! among it, folds the whole log again (`super::refold`). Each field's
//! state is a function of its set of operations (RFC 0004), so folding the
//! parked operations late gives the view a full fold would give.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use rusqlite::Connection;

use super::fold::SqlFold;
use super::log_table::StoredFrames;
use super::model::{Field, Index, Model};
use super::{Error, log_table, schema};
use crate::field_op::Origin;
use crate::ids::SpaceId;
use crate::log::{Log, OperationParkReason, ParkedOperation, State};
use crate::receive::{Checks, Fold, fold_one, understand};
use crate::unfold::{fold_revived, leaves_placeholders, refold_displaced, refold_placed};

/// What a model adds to the model the view's tables were folded by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Additions {
    /// New collections, with their fields.
    collections: BTreeMap<String, BTreeMap<String, Field>>,
    /// New fields of collections the old model knows: collection, name and
    /// field.
    fields: Vec<(String, String, Field)>,
    /// New declared indexes.
    indexes: Vec<Index>,
}

impl Additions {
    /// What `new` adds to `old`, or `None` when it changes or drops any
    /// collection, field or index of `old`.
    pub(crate) fn between(old: &Model, new: &Model) -> Option<Additions> {
        let mut additions = Additions {
            collections: BTreeMap::new(),
            fields: Vec::new(),
            indexes: Vec::new(),
        };
        for (collection, fields) in &old.collections {
            let now = new.collections.get(collection)?;
            for (name, field) in fields {
                if now.get(name) != Some(field) {
                    return None;
                }
            }
        }
        if old.indexes.iter().any(|index| !new.indexes.contains(index)) {
            return None;
        }
        // Another shipped model, or one where there was none, folds the view
        // again: its spaces' changes were parked as of another model.
        if old.reserved != new.reserved {
            return None;
        }
        for (collection, fields) in &new.collections {
            match old.collections.get(collection) {
                None => {
                    additions
                        .collections
                        .insert(collection.clone(), fields.clone());
                }
                Some(known) => additions.fields.extend(
                    fields
                        .iter()
                        .filter(|(name, _)| !known.contains_key(*name))
                        .map(|(name, field)| (collection.clone(), name.clone(), field.clone())),
                ),
            }
        }
        additions.indexes = new
            .indexes
            .iter()
            .filter(|index| !old.indexes.contains(index))
            .cloned()
            .collect();
        Some(additions)
    }

    /// Whether the model adds a collection or a field, whose parked
    /// operations may now fold.
    fn folds(&self) -> bool {
        !self.collections.is_empty() || !self.fields.is_empty()
    }

    /// Whether the model adds a field whose type keeps a rejected change's
    /// operations as placeholders: only rich text does.
    fn keeps_placeholders(&self) -> bool {
        #[cfg(ncdb_rich_text)]
        return self
            .collections
            .values()
            .flat_map(|fields| fields.values())
            .chain(self.fields.iter().map(|(_, _, field)| field))
            .any(|field| matches!(field, Field::RichText(_)));
        #[cfg(not(ncdb_rich_text))]
        false
    }
}

/// Migrates the view's tables to `model` and folds the operations parked as
/// an unknown collection or field, with the checks `checks` gives for each
/// space that holds any. The checks are needed only then; without them,
/// such a view is refused as [`Error::Model`], as a full refold is.
pub(super) fn run<C: Checks>(
    conn: &Connection,
    model: &Model,
    additions: &Additions,
    mut logs: BTreeMap<SpaceId, Log>,
    checks: Option<impl FnMut(SpaceId) -> C>,
) -> Result<(), Error> {
    for (collection, fields) in &additions.collections {
        schema::create_collection(conn, collection, fields)?;
    }
    for (collection, name, field) in &additions.fields {
        schema::add_column(conn, collection, name, field)?;
    }
    for index in &additions.indexes {
        schema::create_index(conn, index)?;
    }
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'model'",
        [schema::hex(&model.hash())],
    )?;
    if !additions.folds() {
        return Ok(());
    }
    let place = additions.keeps_placeholders();
    fold_parked(conn, model, &mut logs, checks, unknown, place)
}

/// Folds the operations parked for a reason `retry` names, in every change
/// of `logs` that holds any, with the checks `checks` gives for each such
/// space; without them, that is [`Error::Model`]. With `place`, a field
/// the model adds that keeps placeholders, also takes those of the
/// changes rejected as not a member, which the fold never opened. An operation that still
/// does not fold stays parked, for the reason it gives now. A take-back
/// among them folds its object's fields again (RFC 0016; `super::trace`).
pub(super) fn fold_parked<C: Checks>(
    conn: &Connection,
    model: &Model,
    logs: &mut BTreeMap<SpaceId, Log>,
    checks: Option<impl FnMut(SpaceId) -> C>,
    retry: fn(OperationParkReason) -> bool,
    place: bool,
) -> Result<(), Error> {
    let mut checks = checks;
    for (space, log) in logs.iter_mut() {
        let waiting: Vec<_> = log
            .folded_sealed()
            .into_iter()
            .filter(|(_, parked)| parked.iter().any(|operation| retry(operation.reason)))
            .collect();
        // A change rejected as not a member was never folded, so a field
        // the model now knows has none of its placeholders yet (RFC 0013,
        // "Un-folding a rejected change").
        let placed: Vec<_> = match place {
            true => log
                .rejected()
                .into_iter()
                .filter(|id| leaves_placeholders(log.state(*id)))
                .collect(),
            false => Vec::new(),
        };
        if waiting.is_empty() && placed.is_empty() {
            continue;
        }
        let factory = checks.as_mut().ok_or(Error::Model)?;
        let mut space_checks = factory(*space);
        let controls: Vec<_> = log.folded_controls().cloned().collect();
        space_checks.add_controls(&controls);
        let mut revived = BTreeSet::new();
        for (id, parked) in waiting {
            let mut frames = StoredFrames::new(conn);
            let change = log.change(id, &mut frames).map(Cow::into_owned);
            frames.finish()?;
            let Some(change) = change else {
                continue;
            };
            // A change that no longer opens keeps its parked operations,
            // as the fold before left them.
            let Ok(payload) = space_checks.open_folded(&change) else {
                continue;
            };
            let mut fold =
                SqlFold::new(conn, model, *space).for_space(space_checks.space_model_id());
            let mut still = Vec::new();
            for operation in parked {
                let position = operation.position;
                let Some(op) = (retry(operation.reason))
                    .then(|| payload.operations.get(position as usize))
                    .flatten()
                else {
                    still.push(operation);
                    continue;
                };
                let origin = Origin {
                    id: change.operation_id(position),
                    author: change.header().author,
                    stamp: payload.clock,
                };
                let folded = understand(op, &fold).and_then(|()| fold_one(op, origin, &mut fold));
                if let Err(reason) = folded {
                    still.push(ParkedOperation { position, reason });
                }
            }
            revived.append(&mut fold.take_revived());
            fold.finish()?;
            log.set_state(change.id(), State::Folded { parked: still });
            log_table::update(conn, log, change.id())?;
        }
        // A take-back among them folds its object's fields again, from
        // every change now folded (RFC 0016; `super::trace`).
        let mut fold = SqlFold::new(conn, model, *space).for_space(space_checks.space_model_id());
        fold.revived = revived;
        refold_placed(log, &space_checks, &mut fold, &placed);
        fold_revived(log, &space_checks, &mut fold);
        refold_displaced(log, &space_checks, &mut fold);
        fold.finish()?;
    }
    Ok(())
}

/// Whether an operation was parked because the model did not know its
/// collection or field, or had no code for its field's type, which an
/// additive change, or a plugin registered at the next open, may now
/// know (RFC 0011, "A type that is not registered").
fn unknown(reason: OperationParkReason) -> bool {
    matches!(
        reason,
        OperationParkReason::UnknownCollection
            | OperationParkReason::UnknownField
            | OperationParkReason::UnknownType
    )
}
