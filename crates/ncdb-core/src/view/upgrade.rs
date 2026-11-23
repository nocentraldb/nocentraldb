//! What a view an older core folded gains when this core opens it (RFC
//! 0016, "Formats": an old core parks the move "as an unknown operation,
//! so it shows the object as live until it upgrades"). An older core
//! parked a move as an unknown operation and a set of `$movedFrom` as an
//! unknown field; the first open by this core folds them, as a migration
//! folds what a new field makes known (`super::migrate`). The view's
//! `meta` then records the object operations it knows, so that later
//! opens open no change for it.
//!
//! It needs the member's checks to open the changes. A view opened without
//! them, by [`View::open`](super::View::open), keeps them parked until an
//! open with checks.

use std::collections::BTreeMap;

use rusqlite::Connection;

use super::Error;
use super::migrate::fold_parked;
use super::model::Model;
use super::schema::meta;
use crate::ids::SpaceId;
use crate::log::{Log, OperationParkReason};
use crate::receive::Checks;

/// The highest object operation this core folds, the move: `meta` holds it
/// under `operations` once the view folded what an older core parked.
const OPERATIONS: &str = "6";

/// Folds what an older core parked and this core knows, unless the view
/// already did, and records that it did. Without `checks`, it does so only
/// when nothing waits.
pub(super) fn run<C: Checks>(
    conn: &Connection,
    model: &Model,
    mut logs: BTreeMap<SpaceId, Log>,
    checks: Option<impl FnMut(SpaceId) -> C>,
) -> Result<(), Error> {
    if meta(conn, "operations")?.as_deref() == Some(OPERATIONS) {
        return Ok(());
    }
    let waits = logs.values().any(|log| {
        log.folded_sealed()
            .iter()
            .any(|(_, parked)| parked.iter().any(|p| newly_known(p.reason)))
    });
    if waits && checks.is_none() {
        return Ok(());
    }
    fold_parked(conn, model, &mut logs, checks, newly_known)?;
    conn.execute(
        "INSERT INTO meta (key, value) VALUES ('operations', ?1)
         ON CONFLICT DO UPDATE SET value = excluded.value",
        [OPERATIONS],
    )?;
    Ok(())
}

/// Whether an operation an older core parked may fold now: an unknown
/// operation, the move, or an unknown field, `$movedFrom`. One this core
/// does not know either parks again, for the same reason.
fn newly_known(reason: OperationParkReason) -> bool {
    matches!(
        reason,
        OperationParkReason::UnknownOperation | OperationParkReason::UnknownField
    )
}
