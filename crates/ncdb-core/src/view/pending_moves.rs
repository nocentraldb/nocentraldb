//! The moves whose second change this device has still to write (RFC
//! 0016, "The new space first"). The SDK writes a move as two changes, the
//! new space's first; it records each object here before it writes them
//! and clears it once the old space's move is written. If the device stops
//! in between, the SDK finds the record when the database opens again and
//! writes the move then.
//!
//! The table `pending_moves` holds one row per object: the old space,
//! `old_space_id`; the object, `object_id`; the new space, `new_space_id`;
//! and its collection. Like `ad_hoc_copies`, it lives in the member's
//! SQLite file beside the view but is no part of it: no fold computes it,
//! so neither folding the log again nor a rebuild touches it, and it never
//! syncs. A database from before the table gains it, empty, the next time
//! it opens (`CREATE TABLE IF NOT EXISTS`), so the view format version
//! stays as it is.

use rusqlite::params;

use super::{Error, View};
use crate::ids::{ObjectId, SpaceId};
use crate::texts;

/// One object whose move the device has still to finish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMove {
    /// The space the object leaves.
    pub old_space: SpaceId,
    /// The space it goes to.
    pub new_space: SpaceId,
    /// Its collection.
    pub collection: String,
    /// The object.
    pub object: ObjectId,
}

impl View {
    /// Records that the objects of `old`, each with its collection, are
    /// moving to `new`, all or none. A record of an object already pending
    /// is replaced.
    pub fn record_pending_moves(
        &mut self,
        old: SpaceId,
        new: SpaceId,
        objects: &[(String, ObjectId)],
    ) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO pending_moves (old_space_id, object_id, new_space_id, collection)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET new_space_id = excluded.new_space_id,
                     collection = excluded.collection",
            )?;
            for (collection, object) in objects {
                insert.execute(params![old.0, object.0, new.0, collection])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Every pending move, by old space and object.
    pub fn pending_moves(&self) -> Result<Vec<PendingMove>, Error> {
        let mut statement = self.conn.prepare(
            "SELECT old_space_id, object_id, new_space_id, collection FROM pending_moves
             ORDER BY old_space_id, object_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (old, object, new, collection) = row?;
            Ok(PendingMove {
                old_space: SpaceId(id(old)?),
                new_space: SpaceId(id(new)?),
                collection,
                object: ObjectId(id(object)?),
            })
        })
        .collect()
    }

    /// Forgets the pending moves of `objects` of `old`.
    pub fn clear_pending_moves(&mut self, old: SpaceId, objects: &[ObjectId]) -> Result<(), Error> {
        let tx = self.conn.transaction()?;
        {
            let mut delete =
                tx.prepare("DELETE FROM pending_moves WHERE old_space_id = ?1 AND object_id = ?2")?;
            for object in objects {
                delete.execute(params![old.0, object.0])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

/// A 16-byte id read back from the table.
fn id(bytes: Vec<u8>) -> Result<[u8; 16], Error> {
    bytes
        .try_into()
        .map_err(|_| Error::Corrupt(texts::UNREADABLE_PENDING_MOVE))
}
