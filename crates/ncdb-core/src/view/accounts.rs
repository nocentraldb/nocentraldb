//! The accounts a person confirmed (NOC-168; RFC 0009, "Invitations and
//! members"): the database remembers each account whose fingerprint a
//! person compared, so that adding another member of it needs no
//! fingerprint again, the way messaging apps compare safety numbers once.
//!
//! The table `confirmed_accounts` holds one row per account: its 32-byte
//! Ed25519 public key, `account_key`, and when a person confirmed it,
//! `confirmed_at`, in milliseconds since the Unix epoch. Nothing else.
//! It lives in the member's SQLite file beside the view, but is no part of
//! it: no fold computes it, so neither folding the log again nor a rebuild
//! touches it. A database from before the table gains it, empty, the next
//! time it opens (`CREATE TABLE IF NOT EXISTS`), and a core that does not
//! know it ignores it, so the view format version stays 2.

use rusqlite::{OptionalExtension, params};

use super::{Error, View};

impl View {
    /// Records that a person confirmed the account with the public key
    /// `account` at `at`, in milliseconds since the Unix epoch. An account
    /// confirmed before keeps the time of its first confirmation.
    pub fn confirm_account(&mut self, account: &[u8; 32], at: u64) -> Result<(), Error> {
        let at = i64::try_from(at).map_err(|_| Error::Corrupt("confirmation time"))?;
        self.conn.execute(
            "INSERT INTO confirmed_accounts (account_key, confirmed_at) VALUES (?1, ?2)
             ON CONFLICT (account_key) DO NOTHING",
            params![account, at],
        )?;
        Ok(())
    }

    /// When a person confirmed the account with the public key `account`,
    /// in milliseconds since the Unix epoch, or `None` when nobody did.
    pub fn account_confirmed_at(&self, account: &[u8; 32]) -> Result<Option<u64>, Error> {
        let at: Option<i64> = self
            .conn
            .query_row(
                "SELECT confirmed_at FROM confirmed_accounts WHERE account_key = ?1",
                [account],
                |row| row.get(0),
            )
            .optional()?;
        at.map(|at| u64::try_from(at).map_err(|_| Error::Corrupt("confirmation time")))
            .transpose()
    }
}
