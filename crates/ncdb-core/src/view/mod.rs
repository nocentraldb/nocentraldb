//! The local view on SQLite (RFC 0006,
//! `docs/rfc/0006-local-view.md`): one database per member, holding the
//! member's log (RFC 0002, "The log on disk") and the view folded from it.
//!
//! A received change is appended to the log, taken through the checks of
//! RFC 0002 by [`Log::receive`], and folded into the tables of
//! its collections, in one SQLite transaction ([`View::receive`]): after a
//! crash a change is in both the log and the view or in neither. The view
//! keeps each space's [`Log`] in memory too, read back from the `log` table
//! when it opens, so the steps of [`crate::receive`] run unchanged.
//!
//! The same code runs natively and on wasm32 (RFC 0006, "The same fold
//! code on every target"): only the SQLite under `rusqlite` differs.

mod accounts;
#[cfg(target_arch = "wasm32")]
pub mod browser;
mod fold;
mod from_lock;
mod log_table;
mod migrate;
pub mod model;
mod query;
mod read;
mod rebuild;
mod refold;
mod relay;
mod schema;
mod state;
mod stored_lock;

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use rusqlite::Connection;

use crate::change::Change;
use crate::checks::SealChecks;
use crate::ids::{ChangeId, SpaceId};
use crate::log::{Appended, Log, State, StateCounts};
use crate::model_lock::{ModelLock, Refusal};
use crate::receive::{Checks, ReceiveError};
use crate::tips::Tips;

pub use model::{Direction, Field, FieldState, Index, Invalid, Model, Scope};
pub use query::{Cursor, Page, QueryError};
pub use read::Row;
pub use rebuild::RebuildProgress;

/// A member's local view: its database, its model and the log of each of
/// its spaces.
pub struct View {
    conn: Connection,
    /// The model the view's tables were folded by, which reads and new
    /// changes use; during a rebuild, the old one.
    model: Model,
    logs: BTreeMap<SpaceId, Log>,
    /// A full rebuild under way (RFC 0006, "How").
    rebuild: Option<rebuild::Rebuild>,
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
    /// The database holds a view folded by another model, which this call
    /// cannot fold again: [`View::open`] has no checks to fold with, or
    /// the database holds no lock to check the new one against.
    Model,
    /// The lock is not a successor of the lock the database was last
    /// opened with, nor that lock one of it (RFC 0005, D11): each refused
    /// change.
    Refused(Vec<Refusal>),
    /// The model cannot be folded into a view.
    Invalid(Invalid),
    /// The database holds something this core did not write.
    Corrupt(&'static str),
    /// A lookup was refused.
    Query(QueryError),
    /// The lookup of this declared index, `<collection>.<index>`, would
    /// not run through it, or
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
            Error::Refused(refused) => {
                f.write_str(
                    "the lock is refused against the lock the database was last opened with:",
                )?;
                for line in refused.iter().flat_map(Refusal::lines) {
                    write!(f, "\n  {line}")?;
                }
                Ok(())
            }
            Error::Invalid(invalid) => write!(f, "the model cannot be folded: {invalid:?}"),
            Error::Corrupt(what) => write!(f, "the database holds an unreadable {what}"),
            Error::Query(error) => write!(f, "the lookup was refused: {error:?}"),
            Error::Plan(index) => write!(f, "the lookup of index {index} does not run through it"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Whether SQLite failed because the disk or the quota is full: the one
    /// failure that freeing space can mend.
    pub fn is_full(&self) -> bool {
        matches!(
            self,
            Error::Sqlite(rusqlite::Error::SqliteFailure(error, _))
                if error.code == rusqlite::ErrorCode::DiskFull
        )
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        Error::Sqlite(error)
    }
}

impl View {
    /// The version of the view's layout. A core that lays out the view
    /// differently writes another version.
    pub const FORMAT: u64 = 2;

    /// Opens the view in the database file at `path`, creating it when it
    /// does not exist: WAL mode, `synchronous = FULL` and `secure_delete`
    /// on (RFC 0006, decisions 7 and 9). The view folds by the model of
    /// `lock`, the member's `model.lock.json` (RFC 0005), which is checked
    /// against the lock the database was last opened with (RFC 0005, D11):
    /// a lock that is not a successor of it, nor it a successor of the
    /// lock, is [`Error::Refused`], and nothing is written. A change the
    /// check accepts that changes the view's model is [`Error::Model`]
    /// here, since folding the log again needs the member's checks: see
    /// [`View::open_with`].
    pub fn open(path: &Path, lock: &ModelLock) -> Result<View, Error> {
        View::with(
            Connection::open(path)?,
            lock,
            None::<fn(SpaceId) -> SealChecks>,
        )
    }

    /// Opens the view as [`View::open`] does, and when the lock check
    /// accepts a change of the view's model, brings the view to the new
    /// model with the checks `checks` gives for each space (RFC 0006,
    /// "Rebuilding the view"). An additive change migrates the view in
    /// place. A full rebuild starts in the background when the model the
    /// view was folded by is known: the view then keeps serving by it, and
    /// [`View::rebuild_step`] runs the rebuild. Otherwise the whole log is
    /// folded again before it returns. If anything fails, the database
    /// stays as it was.
    pub fn open_with<C: Checks + Send + 'static>(
        path: &Path,
        lock: &ModelLock,
        checks: impl FnMut(SpaceId) -> C + Send + 'static,
    ) -> Result<View, Error> {
        View::with(Connection::open(path)?, lock, Some(checks))
    }

    /// Opens a view in a new database in memory, which nothing keeps.
    pub fn open_in_memory(lock: &ModelLock) -> Result<View, Error> {
        View::with(
            Connection::open_in_memory()?,
            lock,
            None::<fn(SpaceId) -> SealChecks>,
        )
    }

    fn with<C: Checks + Send + 'static>(
        conn: Connection,
        lock: &ModelLock,
        checks: Option<impl FnMut(SpaceId) -> C + Send + 'static>,
    ) -> Result<View, Error> {
        let model = Model::from_lock(lock).map_err(Error::Invalid)?;
        // journal_mode answers with the mode it set, so it is a query.
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA secure_delete = ON;")?;
        let tx = conn.unchecked_transaction()?;
        // A rebuild cut off by a close starts again (NOC-227).
        schema::drop_staging(&tx)?;
        let mut serving = model;
        let mut rebuild = None;
        match schema::open(&tx, &serving, lock)? {
            schema::Fit::Current => {}
            schema::Fit::Migrate(additions) => {
                migrate::run(&tx, &serving, &additions, log_table::load(&tx)?, checks)?;
            }
            schema::Fit::Refold(Some(old)) if checks.is_some() => {
                let mut checks = checks.expect("checked above");
                let factory: rebuild::Factory = Box::new(move |space| Box::new(checks(space)));
                rebuild = Some(rebuild::Rebuild::start(&tx, serving, factory)?);
                serving = *old;
            }
            schema::Fit::Refold(_) => {
                let checks = checks.ok_or(Error::Model)?;
                refold::run(&tx, &serving, &log_table::load(&tx)?, checks)?;
            }
        }
        query::check(&tx, &serving.indexes)?;
        let logs = log_table::load(&tx)?;
        tx.commit()?;
        Ok(View {
            conn,
            model: serving,
            logs,
            rebuild,
        })
    }

    /// The model the view folds by: during a rebuild, the one its tables
    /// were folded by, until the rebuild swaps in the new ones.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// How far a rebuild is, or `None` when none runs.
    pub fn rebuild_progress(&self) -> Option<RebuildProgress> {
        Some(self.rebuild.as_ref()?.progress(&self.logs))
    }

    /// Folds up to `frames` more frames of the log into a rebuild's staging
    /// tables, in a transaction of their own, and says how far it is then.
    /// When the rebuild has reached the end of every space's log, the same
    /// transaction swaps the new tables in; the view then folds by the new
    /// model, and this gives `None`, as it does when no rebuild runs.
    pub fn rebuild_step(&mut self, frames: usize) -> Result<Option<RebuildProgress>, Error> {
        let Some(rebuild) = self.rebuild.as_mut() else {
            return Ok(None);
        };
        let tx = self.conn.transaction()?;
        let step = rebuild.fold(&tx, &self.logs, frames).and_then(|()| {
            let progress = rebuild.progress(&self.logs);
            if progress.folded < progress.total {
                return Ok(Some(progress));
            }
            rebuild.swap(&tx).map(|()| None)
        });
        let progress =
            match step.and_then(|progress| tx.commit().map(|()| progress).map_err(Error::from)) {
                Ok(progress) => progress,
                Err(error) => {
                    // The transaction rolled back; the rebuild starts again.
                    rebuild.restart(&self.conn)?;
                    return Err(error);
                }
            };
        if progress.is_none() {
            let rebuild = self.rebuild.take().expect("a rebuild runs");
            self.model = rebuild.into_model();
            self.logs = log_table::load(&self.conn)?;
        }
        Ok(progress)
    }

    /// Receives a frame: appends it to the log of its space, takes it
    /// through the steps and folds it, and, when it is a control change
    /// that passes, checks rejected changes again (D11) and un-folds the
    /// folded changes it now rejects (RFC 0006, "Un-folding"), all in one
    /// SQLite transaction. A duplicate changes nothing. When anything fails,
    /// neither the log nor the view changes.
    pub fn receive(&mut self, bytes: &[u8], checks: &mut impl Checks) -> Result<Appended, Error> {
        self.receive_at(bytes, checks, None)
    }

    /// Receives a change the member wrote itself, as [`View::receive`]
    /// does, and keeps `written_at`, the time it was written in
    /// milliseconds since the Unix epoch, with it in the log (RFC 0002,
    /// "On a member"; NOC-231). A duplicate keeps the time it had.
    pub fn receive_own(
        &mut self,
        bytes: &[u8],
        checks: &mut impl Checks,
        written_at: u64,
    ) -> Result<Appended, Error> {
        self.receive_at(bytes, checks, Some(written_at))
    }

    fn receive_at(
        &mut self,
        bytes: &[u8],
        checks: &mut impl Checks,
        written_at: Option<u64>,
    ) -> Result<Appended, Error> {
        let change =
            Change::decode(bytes).map_err(|error| Error::Receive(ReceiveError::Decode(error)))?;
        let result = self.receive_change(&change, bytes, checks, written_at);
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
        written_at: Option<u64>,
    ) -> Result<Appended, Error> {
        let tx = self.conn.transaction()?;
        let appended = append(
            &tx,
            &self.model,
            &mut self.logs,
            change,
            bytes,
            checks,
            written_at,
        )?;
        tx.commit()?;
        Ok(appended)
    }

    /// Receives every frame of `frames`, in order, as [`View::receive`]
    /// receives one, all in one SQLite transaction: either every frame is
    /// appended and folded, or, when any of them fails, none is, and
    /// neither the log nor the view changes. A crash in the middle leaves
    /// none of them either. Says what became of each frame.
    pub fn receive_all(
        &mut self,
        frames: &[&[u8]],
        checks: &mut impl Checks,
    ) -> Result<Vec<Appended>, Error> {
        self.receive_all_at(frames, checks, None)
    }

    /// Receives every frame of `frames`, changes the member wrote itself,
    /// as [`View::receive_all`] does, and keeps `written_at` with each new
    /// one, as [`View::receive_own`] does.
    pub fn receive_all_own(
        &mut self,
        frames: &[&[u8]],
        checks: &mut impl Checks,
        written_at: u64,
    ) -> Result<Vec<Appended>, Error> {
        self.receive_all_at(frames, checks, Some(written_at))
    }

    fn receive_all_at(
        &mut self,
        frames: &[&[u8]],
        checks: &mut impl Checks,
        written_at: Option<u64>,
    ) -> Result<Vec<Appended>, Error> {
        let result = self.receive_batch(frames, checks, written_at);
        if result.is_err() {
            // The transaction rolled back; the logs in memory follow it.
            self.logs = log_table::load(&self.conn)?;
        }
        result
    }

    fn receive_batch(
        &mut self,
        frames: &[&[u8]],
        checks: &mut impl Checks,
        written_at: Option<u64>,
    ) -> Result<Vec<Appended>, Error> {
        let changes = frames
            .iter()
            .map(|bytes| Change::decode(bytes))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Error::Receive(ReceiveError::Decode(error)))?;
        let tx = self.conn.transaction()?;
        let mut appended = Vec::with_capacity(frames.len());
        for (change, bytes) in changes.iter().zip(frames) {
            appended.push(append(
                &tx,
                &self.model,
                &mut self.logs,
                change,
                bytes,
                checks,
                written_at,
            )?);
        }
        tx.commit()?;
        Ok(appended)
    }

    /// The spaces whose log the view holds, in id order.
    pub fn spaces(&self) -> impl Iterator<Item = SpaceId> + '_ {
        self.logs.keys().copied()
    }

    /// The log of `space`, when the view holds one.
    pub fn log(&self, space: SpaceId) -> Option<&Log> {
        self.logs.get(&space)
    }

    /// The tips of the log of `space`, when the view holds one: what the
    /// member's next change in the space takes from it (NOC-138). The view
    /// reads each change's state back from the `log` table without opening
    /// it, so after the view opens, the first read opens each folded
    /// sealed change once with `checks`; see [`Log::tips`].
    pub fn tips(&mut self, space: SpaceId, checks: &impl Checks) -> Option<&Tips> {
        Some(self.logs.get_mut(&space)?.tips(checks))
    }

    /// The newest model version the parked data of `space` needs, of the
    /// view's own model (RFC 0002, "What the model version is for"): see
    /// [`Log::newest_model`]. `checks` open the changes it reads.
    pub fn newest_model(&self, space: SpaceId, checks: &impl Checks) -> Option<u64> {
        self.logs.get(&space)?.newest_model(checks, self.model.id)
    }

    /// How many changes of `space` are parked and rejected, and how many
    /// operations of its folded changes are parked, per reason (RFC 0009,
    /// "Sync and status"); nothing for a space the view does not hold.
    pub fn state_counts(&self, space: SpaceId) -> StateCounts {
        self.logs
            .get(&space)
            .map(|log| log.state_counts().clone())
            .unwrap_or_default()
    }

    /// The state of a change the log of `space` holds.
    pub fn state(&self, space: SpaceId, id: ChangeId) -> Option<State> {
        self.logs.get(&space)?.state(id).cloned()
    }
}

/// Appends `change` to the log of its space inside the transaction `tx`,
/// takes it through the steps and folds it, and records in `log` the
/// changes whose state a passing control change changed.
fn append(
    tx: &Connection,
    model: &Model,
    logs: &mut BTreeMap<SpaceId, Log>,
    change: &Change,
    bytes: &[u8],
    checks: &mut impl Checks,
    written_at: Option<u64>,
) -> Result<Appended, Error> {
    let space = change.header().space;
    let log = logs.entry(space).or_insert_with(|| Log::new(space));
    log.take_changed();
    let mut fold = fold::SqlFold::new(tx, model, space);
    let appended = log.receive(bytes, checks, &mut fold);
    fold.finish()?;
    let appended = appended.map_err(Error::Receive)?;
    if let Appended::New(_) = appended {
        log_table::insert(tx, log, change.id(), written_at)?;
        // A passing control change can change the state of others.
        for id in log.take_changed() {
            if id != change.id() {
                log_table::update(tx, log, id)?;
            }
        }
    }
    Ok(appended)
}
