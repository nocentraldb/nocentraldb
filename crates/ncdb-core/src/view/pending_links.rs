//! The link statements a completed pairing has yet to write into the
//! account space (RFC 0010, "The link statement"; NOC-267): a database that
//! had an account of its own signs its old key's half when it completes a
//! pairing and deletes the old key, so the whole statement must not be lost
//! before the account space holds it.
//!
//! The table `pending_links` holds one row per statement: its canonical
//! bytes, `statement`, and the change that wrote it into the account space,
//! `change_id`, once one did. It lives in the member's SQLite file beside
//! the view, but is no part of it: no fold computes it, so neither folding
//! the log again nor a rebuild touches it. The SDK deletes a row once the
//! relay confirmed its change. A database from before the table gains it,
//! empty, the next time it opens (`CREATE TABLE IF NOT EXISTS`), so the
//! view format version stays 2.

use rusqlite::params;

use super::{Error, View};
use crate::ids::ChangeId;

/// A kept link statement: its bytes, and the change that wrote it into
/// the account space, once one did.
pub type PendingLink = (Vec<u8>, Option<ChangeId>);

impl View {
    /// Keeps `statement` to be written into the account space. A statement
    /// kept already is kept once.
    pub fn put_pending_link(&mut self, statement: &[u8]) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO pending_links (statement) VALUES (?1) ON CONFLICT (statement) DO NOTHING",
            [statement],
        )?;
        Ok(())
    }

    /// Records the change that wrote `statement` into the account space.
    pub fn set_pending_link_change(
        &mut self,
        statement: &[u8],
        change: ChangeId,
    ) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE pending_links SET change_id = ?2 WHERE statement = ?1",
            params![statement, change.0],
        )?;
        Ok(())
    }

    /// Forgets `statement`, once the relay holds the change that wrote it.
    pub fn delete_pending_link(&mut self, statement: &[u8]) -> Result<(), Error> {
        self.conn.execute(
            "DELETE FROM pending_links WHERE statement = ?1",
            [statement],
        )?;
        Ok(())
    }

    /// The statements kept, in the order of their bytes, each with the
    /// change that wrote it, once one did.
    pub fn pending_links(&self) -> Result<Vec<PendingLink>, Error> {
        let mut query = self
            .conn
            .prepare("SELECT statement, change_id FROM pending_links ORDER BY statement")?;
        let rows = query.query_map([], |row| {
            Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
        })?;
        let mut links = Vec::new();
        for row in rows {
            let (statement, change) = row?;
            let change = change
                .map(|bytes| {
                    bytes
                        .try_into()
                        .map(ChangeId)
                        .map_err(|_| Error::Corrupt("pending link change id"))
                })
                .transpose()?;
            links.push((statement, change));
        }
        Ok(links)
    }
}
