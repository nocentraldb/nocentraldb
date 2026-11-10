//! The relay's answers in the member's log (RFC 0002, "The log on disk";
//! RFC 0007, "Island mode"): the relay position of each change the relay
//! holds, and the relay's refusal of a change (RFC 0002, D12), in the
//! `log` table's `relay_position` and `relay_refusal` columns. Neither
//! changes a change's state: no other member sees the relay's answer.

use rusqlite::{OptionalExtension, params};

use super::{Error, View};
use crate::change::Change;
use crate::ids::{ChangeId, SpaceId};
use crate::log::RelayRefusal;

impl View {
    /// The changes of `space` the relay has neither confirmed nor refused,
    /// in local order: what a member pushes (RFC 0007, "Island mode",
    /// step 2).
    pub fn unconfirmed(&self, space: SpaceId) -> Result<Vec<Change>, Error> {
        self.frames(
            "SELECT frame FROM log WHERE space_id = ?1
             AND relay_position IS NULL AND relay_refusal IS NULL
             ORDER BY local_position",
            space,
        )
    }

    /// The changes of `space` the relay refused, in local order, with the
    /// relay's code.
    pub fn refused(&self, space: SpaceId) -> Result<Vec<(Change, RelayRefusal)>, Error> {
        let mut statement = self.conn.prepare(
            "SELECT frame, relay_refusal FROM log WHERE space_id = ?1
             AND relay_refusal IS NOT NULL ORDER BY local_position",
        )?;
        let mut rows = statement.query([space.0])?;
        let mut refused = Vec::new();
        while let Some(row) = rows.next()? {
            let frame: Vec<u8> = row.get(0)?;
            let change = Change::decode(&frame).map_err(|_| Error::Corrupt("log frame"))?;
            refused.push((change, RelayRefusal(row.get(1)?)));
        }
        Ok(refused)
    }

    /// The relay position of a change, once the relay confirmed it.
    pub fn relay_position(&self, space: SpaceId, id: ChangeId) -> Option<u64> {
        self.conn
            .query_row(
                "SELECT relay_position FROM log WHERE space_id = ?1 AND change_id = ?2",
                params![space.0, id.0],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten()
            .and_then(|position| u64::try_from(position).ok())
    }

    /// The relay's refusal of a change, when one was recorded.
    pub fn relay_refusal(&self, space: SpaceId, id: ChangeId) -> Option<RelayRefusal> {
        self.logs.get(&space)?.relay_refusal(id).cloned()
    }

    /// Records the relay position of a change the log holds: the relay
    /// accepted it there, or already held it there.
    pub fn confirm(&mut self, space: SpaceId, id: ChangeId, position: u64) -> Result<(), Error> {
        let position = i64::try_from(position).map_err(|_| Error::Corrupt("relay position"))?;
        self.conn.execute(
            "UPDATE log SET relay_position = ?3 WHERE space_id = ?1 AND change_id = ?2",
            params![space.0, id.0, position],
        )?;
        Ok(())
    }

    /// Records the relay's refusal of a change the log holds, by its RFC
    /// 0007 code. The change keeps its state and stays folded (D12).
    pub fn refuse(&mut self, space: SpaceId, id: ChangeId, code: &str) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE log SET relay_refusal = ?3 WHERE space_id = ?1 AND change_id = ?2",
            params![space.0, id.0, code],
        )?;
        if let Some(log) = self.logs.get_mut(&space) {
            log.record_relay_refusal(id, RelayRefusal(code.to_owned()));
        }
        Ok(())
    }

    /// Forgets every relay position, of every space: the relay's epoch
    /// changed, so no position it gave holds any more (RFC 0007, "Restores
    /// and the relay epoch"). Refusals stay.
    pub fn forget_relay_positions(&mut self) -> Result<(), Error> {
        self.conn
            .execute("UPDATE log SET relay_position = NULL", [])?;
        Ok(())
    }

    fn frames(&self, sql: &str, space: SpaceId) -> Result<Vec<Change>, Error> {
        let mut statement = self.conn.prepare(sql)?;
        let mut rows = statement.query([space.0])?;
        let mut changes = Vec::new();
        while let Some(row) = rows.next()? {
            let frame: Vec<u8> = row.get(0)?;
            changes.push(Change::decode(&frame).map_err(|_| Error::Corrupt("log frame"))?);
        }
        Ok(changes)
    }
}
