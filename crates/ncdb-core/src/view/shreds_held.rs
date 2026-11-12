//! When this device first held each provisional shred (RFC 0008, "A
//! provisional erasure in a space with one account"; RFC 0009, "Sync and
//! status": `provisionalErasures`). A control change carries no clock (RFC
//! 0008, "The control bodies"), so a shred's age, and the 7 days after
//! which a device of the account finalizes it, count from when this device
//! first held it (the owner's NOC-218 A).
//!
//! The table `shreds_held` holds one row per space and shred: `space_id`,
//! `change_id` and `held_at`, in milliseconds since the Unix epoch. Like
//! `confirmed_accounts`, it lives in the member's SQLite file beside the
//! view but is no part of it: no fold computes it, so neither folding the
//! log again nor a rebuild touches it. A database from before the table
//! gains it, empty, the next time it opens (`CREATE TABLE IF NOT EXISTS`),
//! and a core that does not know it ignores it, so the view format version
//! stays as it is.

use rusqlite::{OptionalExtension, params};

use super::{Error, View};
use crate::ids::{ChangeId, SpaceId};

impl View {
    /// Records that this device held the shred `shred` of `space` at `at`,
    /// in milliseconds since the Unix epoch. A shred held before keeps the
    /// time it was first held.
    pub fn note_shred_held(
        &mut self,
        space: SpaceId,
        shred: ChangeId,
        at: u64,
    ) -> Result<(), Error> {
        let at = i64::try_from(at).map_err(|_| Error::Corrupt("shred time"))?;
        self.conn.execute(
            "INSERT INTO shreds_held (space_id, change_id, held_at) VALUES (?1, ?2, ?3)
             ON CONFLICT (space_id, change_id) DO NOTHING",
            params![space.0, shred.0, at],
        )?;
        Ok(())
    }

    /// When this device first held the shred `shred` of `space`, in
    /// milliseconds since the Unix epoch, or `None` when it never noted it.
    pub fn shred_held_at(&self, space: SpaceId, shred: ChangeId) -> Result<Option<u64>, Error> {
        let at: Option<i64> = self
            .conn
            .query_row(
                "SELECT held_at FROM shreds_held WHERE space_id = ?1 AND change_id = ?2",
                params![space.0, shred.0],
                |row| row.get(0),
            )
            .optional()?;
        at.map(|at| u64::try_from(at).map_err(|_| Error::Corrupt("shred time")))
            .transpose()
    }
}
