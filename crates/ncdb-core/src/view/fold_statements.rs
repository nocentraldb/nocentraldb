//! The SQL statements a [`SqlFold`](super::fold::SqlFold) runs against the
//! view's tables, and what they cost (NOC-542). Every statement of the fold
//! goes through [`Statements`], so [`FoldWork`] counts them all.

use std::cell::Cell;

use rusqlite::{Connection, Params, Row};

use super::Error;

/// What the folds of the last [`View::receive`](super::View::receive),
/// or of another call that receives changes, did to the view's tables:
/// for the cost tests, which hold a write's work per object to a bound.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FoldWork {
    /// The SQL statements the folds prepared.
    pub prepared: usize,
    /// The reads of an object's state from the `objects` table.
    pub object_reads: usize,
}

impl FoldWork {
    /// Adds `other`'s work to this one.
    pub(super) fn add(&mut self, other: FoldWork) {
        self.prepared += other.prepared;
        self.object_reads += other.object_reads;
    }
}

/// The statements of one fold, on the connection of its transaction.
pub(super) struct Statements<'a> {
    conn: &'a Connection,
    prepared: Cell<usize>,
    object_reads: Cell<usize>,
}

impl<'a> Statements<'a> {
    pub(super) fn new(conn: &'a Connection) -> Statements<'a> {
        Statements {
            conn,
            prepared: Cell::new(0),
            object_reads: Cell::new(0),
        }
    }

    /// Runs `sql` with `params`; gives the rows it changed.
    pub(super) fn execute(&self, sql: &str, params: impl Params) -> Result<usize, Error> {
        self.prepared.set(self.prepared.get() + 1);
        Ok(self.conn.prepare(sql)?.execute(params)?)
    }

    /// Runs the query `sql` with `params`; gives `f` of its first row, or
    /// `None` when it has none.
    pub(super) fn query_row<T>(
        &self,
        sql: &str,
        params: impl Params,
        f: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, Error> {
        self.prepared.set(self.prepared.get() + 1);
        let mut statement = self.conn.prepare(sql)?;
        let mut rows = statement.query(params)?;
        Ok(match rows.next()? {
            Some(row) => Some(f(row)?),
            None => None,
        })
    }

    /// Counts a read of an object's state.
    pub(super) fn count_object_read(&self) {
        self.object_reads.set(self.object_reads.get() + 1);
    }

    /// What the fold did so far.
    pub(super) fn work(&self) -> FoldWork {
        FoldWork {
            prepared: self.prepared.get(),
            object_reads: self.object_reads.get(),
        }
    }
}
