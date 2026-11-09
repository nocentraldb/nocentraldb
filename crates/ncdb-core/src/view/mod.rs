//! The local view on SQLite, natively (RFC 0006,
//! `docs/rfc/0006-local-view.md`): one database per member, holding the
//! member's log (RFC 0002, "The log on disk") and the view folded from it.
//!
//! A received change is appended to the log, taken through the checks of
//! RFC 0002 Amendment 1 by [`Log::receive`], and folded into the tables of
//! its collections, in one SQLite transaction ([`View::receive`]): after a
//! crash a change is in both the log and the view or in neither. The view
//! keeps each space's [`Log`] in memory too, read back from the `log` table
//! when it opens, so the steps of [`crate::receive`] run unchanged.
//!
//! Native only for now: the browser's SQLite on OPFS is a later ticket.

mod fold;
mod log_table;
pub mod model;
mod query;
mod read;
mod schema;
mod state;

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use rusqlite::Connection;

use crate::change::Change;
use crate::ids::{ChangeId, SpaceId};
use crate::log::{Appended, Log, State};
use crate::receive::{Checks, ReceiveError};

pub use model::{Direction, Field, FieldState, Index, Invalid, Model};
pub use query::{Cursor, Page, QueryError};
pub use read::Row;

/// A member's local view: its database, its model and the log of each of
/// its spaces.
pub struct View {
    conn: Connection,
    model: Model,
    logs: BTreeMap<SpaceId, Log>,
}

impl fmt::Debug for View {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("spaces", &self.logs.len())
            .finish()
    }
}

/// Why the view could not be opened, or a change not received.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// SQLite failed.
    Sqlite(rusqlite::Error),
    /// The change was not appended.
    Receive(ReceiveError),
    /// The database holds a view of another format version, which a
    /// rebuild would replace (a later ticket).
    Format {
        /// The version the database holds.
        found: u64,
    },
    /// The database holds a view folded by another model.
    Model,
    /// The model cannot be folded into a view.
    Invalid(Invalid),
    /// The database holds something this core did not write.
    Corrupt(&'static str),
    /// A lookup was refused.
    Query(QueryError),
    /// The lookup of this declared index would not run through it, or
    /// would sort in a temporary B-tree (RFC 0006, "The check").
    Plan(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Sqlite(error) => write!(f, "SQLite failed: {error}"),
            Error::Receive(error) => error.fmt(f),
            Error::Format { found } => write!(
                f,
                "the view has format version {found}, this core writes {}",
                View::FORMAT
            ),
            Error::Model => f.write_str("the view was folded by another model"),
            Error::Invalid(invalid) => write!(f, "the model cannot be folded: {invalid:?}"),
            Error::Corrupt(what) => write!(f, "the database holds an unreadable {what}"),
            Error::Query(error) => write!(f, "the lookup was refused: {error:?}"),
            Error::Plan(index) => write!(f, "the lookup of index {index} does not run through it"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        Error::Sqlite(error)
    }
}

impl View {
    /// The version of the view's layout. A core that lays out the view
    /// differently writes another version.
    pub const FORMAT: u64 = 1;

    /// Opens the view in the database file at `path`, creating it when it
    /// does not exist: WAL mode, `synchronous = FULL` and `secure_delete`
    /// on (RFC 0006, decisions 7 and 9).
    pub fn open(path: &Path, model: Model) -> Result<View, Error> {
        View::with(Connection::open(path)?, model)
    }

    /// Opens a view in a new database in memory, which nothing keeps.
    pub fn open_in_memory(model: Model) -> Result<View, Error> {
        View::with(Connection::open_in_memory()?, model)
    }

    fn with(conn: Connection, model: Model) -> Result<View, Error> {
        model.validate().map_err(Error::Invalid)?;
        // journal_mode answers with the mode it set, so it is a query.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA secure_delete = ON;")?;
        let tx = conn.unchecked_transaction()?;
        schema::open(&tx, &model)?;
        query::check(&tx, &model.indexes)?;
        let logs = log_table::load(&tx)?;
        tx.commit()?;
        Ok(View { conn, model, logs })
    }

    /// The model the view folds by.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Receives a frame: appends it to the log of its space, takes it
    /// through the steps and folds it, and checks rejected changes again
    /// when it is a control change that passes (D11), all in one SQLite
    /// transaction. A duplicate changes nothing. When anything fails,
    /// neither the log nor the view changes.
    pub fn receive(&mut self, bytes: &[u8], checks: &mut impl Checks) -> Result<Appended, Error> {
        let change =
            Change::decode(bytes).map_err(|error| Error::Receive(ReceiveError::Decode(error)))?;
        let result = self.receive_change(&change, bytes, checks);
        if result.is_err() {
            // The transaction rolled back; the logs in memory follow it.
            self.logs = log_table::load(&self.conn)?;
        }
        result
    }

    fn receive_change(
        &mut self,
        change: &Change,
        bytes: &[u8],
        checks: &mut impl Checks,
    ) -> Result<Appended, Error> {
        let space = change.header().space;
        let log = self.logs.entry(space).or_insert_with(|| Log::new(space));
        // The changes whose state a passing control change can change.
        let rejected: Vec<ChangeId> = log.rejected().iter().map(Change::id).collect();
        let tx = self.conn.transaction()?;
        let mut fold = fold::SqlFold::new(&tx, &self.model, space);
        let appended = log.receive(bytes, checks, &mut fold);
        fold.finish()?;
        let appended = appended.map_err(Error::Receive)?;
        if let Appended::New(_) = appended {
            log_table::insert(&tx, log, change.id())?;
            for id in rejected {
                log_table::update(&tx, log, id)?;
            }
        }
        tx.commit()?;
        Ok(appended)
    }

    /// The state of a change the log of `space` holds.
    pub fn state(&self, space: SpaceId, id: ChangeId) -> Option<State> {
        self.logs.get(&space)?.state(id).cloned()
    }
}
