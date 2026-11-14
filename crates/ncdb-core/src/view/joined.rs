//! Whether the database joined an account another device created (RFC
//! 0010, "The account space"; the owner's NOC-312 C3, Carsten,
//! 2026-11-14): a device that completes a pairing keeps one row that says
//! so, until the account's account space reaches it. Before that, a
//! database opened again would not know it joined an account, and would
//! create an account space of its own beside the account's.
//!
//! The table `joined` holds at most one row. It lives in the member's
//! SQLite file beside the view, but is no part of it: no fold computes it,
//! so neither folding the log again nor a rebuild touches it. A database
//! from before the table gains it, empty, the next time it opens
//! (`CREATE TABLE IF NOT EXISTS`), so the view format version stays 2.

use super::{Error, View};

impl View {
    /// Keeps that the database joined an account another device created.
    /// Keeping it again keeps one row.
    pub fn set_joined(&mut self) -> Result<(), Error> {
        self.conn.execute(
            "INSERT INTO joined (id) VALUES (1) ON CONFLICT (id) DO NOTHING",
            [],
        )?;
        Ok(())
    }

    /// Forgets it, once the account's account space reached the database,
    /// or when the pairing that set it did not complete.
    pub fn clear_joined(&mut self) -> Result<(), Error> {
        self.conn.execute("DELETE FROM joined", [])?;
        Ok(())
    }

    /// Whether the database keeps that it joined an account.
    pub fn joined(&self) -> Result<bool, Error> {
        let rows: i64 = self
            .conn
            .query_row("SELECT count(*) FROM joined", [], |row| row.get(0))?;
        Ok(rows > 0)
    }
}
