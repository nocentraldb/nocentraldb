//! Which object each copy in an ad-hoc space came from (NOC-318; RFC 0003,
//! "Sub-agents get an ad-hoc space"; RFC 0009, "Sub-agents and ad-hoc
//! spaces"). A parent copies objects into an ad-hoc space as new objects,
//! with fresh ids, so that the sub-agent never learns a source's id; the
//! parent's database keeps, on the parent only, which object each copy
//! came from, which is all the parent needs to write results back.
//!
//! The table `ad_hoc_copies` holds one row per copy: the ad-hoc space,
//! `space_id`; the copy, `copy_id`; and its source, `source_space_id` and
//! `source_id`, each the 16 bytes of the id. An ad-hoc space holds at most
//! one copy of a source. Like `confirmed_accounts`, it lives in the
//! member's SQLite file beside the view but is no part of it: no fold
//! computes it, so neither folding the log again nor a rebuild touches it,
//! and it never syncs. A database from before the table gains it, empty,
//! the next time it opens (`CREATE TABLE IF NOT EXISTS`), and a core that
//! does not know it ignores it, so the view format version stays as it is.

use rusqlite::{OptionalExtension, params};

use super::{Error, View};
use crate::ids::{ObjectId, SpaceId};

/// One copy in an ad-hoc space, and the object it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdHocCopy {
    /// The copy, an object of the ad-hoc space.
    pub copy: ObjectId,
    /// The space of the object it came from.
    pub source_space: SpaceId,
    /// The object it came from.
    pub source: ObjectId,
}

impl View {
    /// Records the copies of the ad-hoc space `space`, all or none: a copy
    /// the space holds already, or a second copy of a source it holds a
    /// copy of, fails the whole batch.
    pub fn record_ad_hoc_copies(
        &mut self,
        space: SpaceId,
        copies: &[AdHocCopy],
    ) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO ad_hoc_copies (space_id, copy_id, source_space_id, source_id)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for copy in copies {
                insert.execute(params![
                    space.0,
                    copy.copy.0,
                    copy.source_space.0,
                    copy.source.0
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The space and id of the object that `copy`, in the ad-hoc space
    /// `space`, came from, or `None` when it is no copy of this database.
    pub fn ad_hoc_source_of(
        &self,
        space: SpaceId,
        copy: ObjectId,
    ) -> Result<Option<(SpaceId, ObjectId)>, Error> {
        let row: Option<(Vec<u8>, Vec<u8>)> = self
            .conn
            .query_row(
                "SELECT source_space_id, source_id FROM ad_hoc_copies
                 WHERE space_id = ?1 AND copy_id = ?2",
                params![space.0, copy.0],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(source_space, source)| Ok((SpaceId(id(source_space)?), ObjectId(id(source)?))))
            .transpose()
    }

    /// The copy, in the ad-hoc space `space`, of the object `source` of
    /// `source_space`, or `None` when the space holds no copy of it.
    pub fn ad_hoc_copy_of(
        &self,
        space: SpaceId,
        source_space: SpaceId,
        source: ObjectId,
    ) -> Result<Option<ObjectId>, Error> {
        let copy: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT copy_id FROM ad_hoc_copies
                 WHERE space_id = ?1 AND source_space_id = ?2 AND source_id = ?3",
                params![space.0, source_space.0, source.0],
                |row| row.get(0),
            )
            .optional()?;
        copy.map(|copy| Ok(ObjectId(id(copy)?))).transpose()
    }
}

/// A 16-byte id read back from the table.
fn id(bytes: Vec<u8>) -> Result<[u8; 16], Error> {
    bytes
        .try_into()
        .map_err(|_| Error::Corrupt("an ad-hoc copy's id"))
}
