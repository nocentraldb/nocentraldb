//! The SQL statements a [`SqlFold`](super::fold::SqlFold) runs against the
//! view's tables, and what they cost (NOC-542). Every statement of the fold
//! goes through [`Statements`], so [`FoldWork`] counts them all.
//!
//! A fold runs the same few statements for every operation of a change,
//! and parsing one is about as dear as running it, so [`Statements`]
//! prepares each SQL text once per fold and keeps it until the fold ends.
//! The fold creates and drops no table while it runs, so a kept statement
//! never outlives what it names; SQLite prepares one again by itself if
//! the schema changes all the same.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use rusqlite::{Connection, Params, Row, Statement};

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
    /// Each statement prepared so far, by its SQL text.
    kept: RefCell<HashMap<String, Statement<'a>>>,
    prepared: Cell<usize>,
    object_reads: Cell<usize>,
}

impl<'a> Statements<'a> {
    pub(super) fn new(conn: &'a Connection) -> Statements<'a> {
        Statements {
            conn,
            kept: RefCell::new(HashMap::new()),
            prepared: Cell::new(0),
            object_reads: Cell::new(0),
        }
    }

    /// Runs `f` on the statement of `sql`, prepared on its first use.
    fn with<T>(
        &self,
        sql: &str,
        f: impl FnOnce(&mut Statement<'a>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut kept = self.kept.borrow_mut();
        if !kept.contains_key(sql) {
            self.prepared.set(self.prepared.get() + 1);
            kept.insert(sql.to_owned(), self.conn.prepare(sql)?);
        }
        f(kept.get_mut(sql).expect("prepared above"))
    }

    /// Runs `sql` with `params`; gives the rows it changed.
    pub(super) fn execute(&self, sql: &str, params: impl Params) -> Result<usize, Error> {
        self.with(sql, |statement| Ok(statement.execute(params)?))
    }

    /// Runs the query `sql` with `params`; gives `f` of its first row, or
    /// `None` when it has none.
    pub(super) fn query_row<T>(
        &self,
        sql: &str,
        params: impl Params,
        f: impl FnOnce(&Row<'_>) -> rusqlite::Result<T>,
    ) -> Result<Option<T>, Error> {
        self.with(sql, |statement| {
            let mut rows = statement.query(params)?;
            Ok(match rows.next()? {
                Some(row) => Some(f(row)?),
                None => None,
            })
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
