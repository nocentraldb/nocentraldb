//! A full rebuild of the view in the background (RFC 0006, "Rebuilding the
//! view", "How"; RFC 0009, "What `nc.open` does", step 4; NOC-156). Opening
//! returns at once. The view keeps serving reads and folding new changes
//! with the model its tables were folded by, while the rebuild folds every
//! frame of the log, in local-position order and a page at a time, each
//! page in its own transaction, into staging tables beside the view's
//! (prefix `r_`, NOC-227). When it has reached the end of every space's
//! log, one transaction drops the old tables and renames the staging ones
//! in their place. It also creates the new model's indexes, writes the
//! rebuilt state of every change and updates `meta`. Readers see the new
//! view at once and completely.
//!
//! No marker is kept (NOC-227): a rebuild cut off by a close leaves
//! staging tables, which the next open drops before it starts the rebuild
//! again. It runs only when the model the old tables were folded by is
//! known, the stored lock's, as after an app was rolled back to an older
//! lock. Otherwise the old tables cannot serve until the swap, and the
//! whole log is folded again inside the open, as before
//! (`super::refold`).

use std::collections::BTreeMap;

use rusqlite::Connection;

use super::fold::SqlFold;
use super::model::Model;
use super::{Error, log_table, query, schema};
use crate::ids::SpaceId;
use crate::log::{Log, Position};
use crate::receive::Checks;

/// How far a rebuild is, in frames of the log: `folded` of `total`. The
/// total grows while changes arrive during the rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebuildProgress {
    /// The frames the rebuild has folded.
    pub folded: u64,
    /// The frames of the log.
    pub total: u64,
}

/// Gives the checks of a space, which know no control change yet.
pub(super) type Factory = Box<dyn FnMut(SpaceId) -> Box<dyn Checks + Send> + Send>;

/// A rebuild under way: the model it folds by, and per space the log it
/// rebuilds, its checks and how many frames of the view's log it took.
pub(super) struct Rebuild {
    model: Model,
    factory: Factory,
    spaces: BTreeMap<SpaceId, Staged>,
}

struct Staged {
    log: Log,
    checks: Box<dyn Checks + Send>,
    taken: usize,
}

impl Rebuild {
    /// Starts a rebuild by `model`: drops what a cut-off rebuild left and
    /// creates the staging tables, empty.
    pub(super) fn start(
        conn: &Connection,
        model: Model,
        factory: Factory,
    ) -> Result<Rebuild, Error> {
        schema::drop_staging(conn)?;
        schema::create_staging(conn, &model)?;
        Ok(Rebuild {
            model,
            factory,
            spaces: BTreeMap::new(),
        })
    }

    /// How far the rebuild is against the view's logs `logs`.
    pub(super) fn progress(&self, logs: &BTreeMap<SpaceId, Log>) -> RebuildProgress {
        let total = logs.values().map(Log::len).sum::<usize>();
        let folded = self
            .spaces
            .values()
            .map(|staged| staged.taken)
            .sum::<usize>();
        RebuildProgress {
            folded: folded as u64,
            total: total as u64,
        }
    }

    /// Folds up to `frames` frames of `logs` into the staging tables, in
    /// the transaction `conn`.
    pub(super) fn fold(
        &mut self,
        conn: &Connection,
        logs: &BTreeMap<SpaceId, Log>,
        frames: usize,
    ) -> Result<(), Error> {
        let mut budget = frames;
        for (space, live) in logs {
            if budget == 0 {
                break;
            }
            let factory = &mut self.factory;
            let staged = self.spaces.entry(*space).or_insert_with(|| Staged {
                log: Log::new(*space),
                checks: factory(*space),
                taken: 0,
            });
            while budget > 0 && staged.taken < live.len() {
                let position = Position(staged.taken as u64 + 1);
                let change = live.get(position).expect("the log holds its positions");
                let mut fold = SqlFold::staged(conn, &self.model, *space)
                    .for_space(staged.checks.space_model_id());
                let received = staged
                    .log
                    .receive(change.bytes(), &mut staged.checks, &mut fold);
                fold.finish()?;
                received.map_err(Error::Receive)?;
                staged.taken += 1;
                budget -= 1;
            }
        }
        Ok(())
    }

    /// Starts again from nothing: what a page that failed folded is in the
    /// rebuild's logs but not in its tables, which rolled back.
    pub(super) fn restart(&mut self, conn: &Connection) -> Result<(), Error> {
        self.spaces.clear();
        schema::drop_staging(conn)?;
        schema::create_staging(conn, &self.model)
    }

    /// Swaps the staging tables in, in the transaction `conn`, and writes the
    /// rebuilt state of every change.
    pub(super) fn swap(&self, conn: &Connection) -> Result<(), Error> {
        schema::swap_staging(conn, &self.model)?;
        for staged in self.spaces.values() {
            for (_, change) in staged.log.after(Position::START) {
                log_table::update(conn, &staged.log, change.id())?;
            }
        }
        query::check(conn, &self.model.indexes)
    }

    /// The model the rebuild folds by, which the view folds by once it
    /// swapped the new tables in.
    pub(super) fn into_model(self) -> Model {
        self.model
    }
}
