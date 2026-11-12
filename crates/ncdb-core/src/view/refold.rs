//! Folding the log again after the model changed (RFC 0006, "Rebuilding
//! the view"; RFC 0009, "What `nc.open` does", step 4): new, empty tables
//! for the new model, into which every change of the log is taken through
//! the steps again, in local order, in the transaction that opens the
//! view. What the old model parked and the new one knows is then folded,
//! and what the new model no longer knows, after an app was rolled back,
//! is parked; the log's frames, order and relay positions stay as they
//! were.
//!
//! Each field's state is a function of its set of operations (RFC 0004),
//! so this gives the view an incremental fold under the new model would
//! have given. It folds the whole log every time, where RFC 0006 adds a
//! column for an additive change and folds only the parked operations: the
//! cheaper path is a later ticket, and so is reporting progress while the
//! app reads the old view.

use std::collections::BTreeMap;

use rusqlite::Connection;

use super::fold::SqlFold;
use super::model::Model;
use super::{Error, log_table, schema};
use crate::ids::SpaceId;
use crate::log::{Log, Position};
use crate::receive::Checks;

/// Folds every change of `logs` again into new tables for `model`, with
/// the checks `checks` gives for each space, which know no control change
/// yet, and writes each change's new state into the `log` table.
pub(super) fn run<C: Checks>(
    conn: &Connection,
    model: &Model,
    logs: &BTreeMap<SpaceId, Log>,
    mut checks: impl FnMut(SpaceId) -> C,
) -> Result<(), Error> {
    schema::replace(conn, model)?;
    for (space, old) in logs {
        let mut log = Log::new(*space);
        let mut checks = checks(*space);
        for (_, change) in old.after(Position::START) {
            let mut fold = SqlFold::new(conn, model, *space).for_space(checks.space_model_id());
            let received = log.receive(change.bytes(), &mut checks, &mut fold);
            fold.finish()?;
            received.map_err(Error::Receive)?;
        }
        for (_, change) in log.after(Position::START) {
            log_table::update(conn, &log, change.id())?;
        }
    }
    Ok(())
}
