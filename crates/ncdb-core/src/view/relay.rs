//! The relay's answers in the member's log (RFC 0002, "The log on disk";
//! RFC 0007, "Island mode"): the relay position of each change the relay
//! holds, and the relay's refusal of a change (RFC 0002, D12), in the
//! `log` table's `relay_position` and `relay_refusal` columns. Neither
//! changes a change's state: no other member sees the relay's answer.
//!
//! The relay epoch those positions belong to sits in `meta` under
//! `relay_epoch` (NOC-145), so a reopened database knows whether its
//! positions still hold (RFC 0007, "Restores and the relay epoch"). A
//! space's cursor is derived from the positions, not stored.

use rusqlite::{OptionalExtension, params};

use super::schema;
use super::{Error, View};
use crate::change::Change;
use crate::ids::MemberId;
use crate::ids::{ChangeId, SpaceId};
use crate::log::{RelayCounts, RelayRefusal};

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

    /// How many of `author`'s changes in `space` the relay has neither
    /// confirmed nor refused, and how many it refused (NOC-147): what
    /// [`View::unconfirmed`] and [`View::refused`] hold of that author,
    /// kept as changes are received, confirmed and refused, so reading
    /// them decodes nothing.
    pub fn relay_counts(&self, space: SpaceId, author: MemberId) -> RelayCounts {
        self.logs
            .get(&space)
            .map(|log| log.relay_counts(author))
            .unwrap_or_default()
    }

    /// Records the relay position of a change the log holds: the relay
    /// accepted it there, or already held it there.
    pub fn confirm(&mut self, space: SpaceId, id: ChangeId, position: u64) -> Result<(), Error> {
        let position = i64::try_from(position).map_err(|_| Error::Corrupt("relay position"))?;
        self.conn.execute(
            "UPDATE log SET relay_position = ?3 WHERE space_id = ?1 AND change_id = ?2",
            params![space.0, id.0, position],
        )?;
        if let Some(log) = self.logs.get_mut(&space) {
            log.record_relay_confirmation(id);
        }
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

    /// Forgets the relay's refusal of a change the log holds, because the
    /// member's own fold newly accepts it and pushes it to the relay once
    /// more (RFC 0008, M19). The change keeps its state; until the relay
    /// answers again, it is unconfirmed, unless the relay already holds it.
    pub fn clear_refusal(&mut self, space: SpaceId, id: ChangeId) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE log SET relay_refusal = NULL WHERE space_id = ?1 AND change_id = ?2",
            params![space.0, id.0],
        )?;
        if let Some(log) = self.logs.get_mut(&space) {
            log.clear_relay_refusal(id);
        }
        Ok(())
    }

    /// The relay epoch the relay positions in the log belong to, once
    /// one was set.
    pub fn relay_epoch(&self) -> Result<Option<[u8; 16]>, Error> {
        let Some(text) = schema::meta(&self.conn, RELAY_EPOCH)? else {
            return Ok(None);
        };
        let mut epoch = [0; 16];
        let ok = text.len() == 32
            && epoch.iter_mut().enumerate().all(|(i, byte)| {
                u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
                    .map(|b| *byte = b)
                    .is_ok()
            });
        match ok {
            true => Ok(Some(epoch)),
            false => Err(Error::Corrupt("relay epoch")),
        }
    }

    /// Keeps `epoch` as the relay epoch the relay positions belong to.
    pub fn set_relay_epoch(&mut self, epoch: [u8; 16]) -> Result<(), Error> {
        let text: String = epoch.iter().map(|b| format!("{b:02x}")).collect();
        self.conn.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)
             ON CONFLICT DO UPDATE SET value = excluded.value",
            [RELAY_EPOCH, &text],
        )?;
        Ok(())
    }

    /// The cursor of `space`: the highest relay position up to which the
    /// log holds the change of every position, or 0 when it holds no
    /// change at position one. It is derived, not stored, so it can be
    /// lower than the highest position folded (when a pulled entry never
    /// reached the log) but never higher: a member that pulls from it
    /// skips no change.
    pub fn relay_cursor(&self, space: SpaceId) -> Result<u64, Error> {
        let mut statement = self.conn.prepare(
            "SELECT relay_position FROM log WHERE space_id = ?1
             AND relay_position IS NOT NULL ORDER BY relay_position",
        )?;
        let mut rows = statement.query([space.0])?;
        let mut cursor = 0u64;
        while let Some(row) = rows.next()? {
            let position = u64::try_from(row.get::<_, i64>(0)?)
                .map_err(|_| Error::Corrupt("relay position"))?;
            match position {
                p if p <= cursor => {}
                p if p == cursor + 1 => cursor = p,
                _ => break,
            }
        }
        Ok(cursor)
    }

    /// Forgets every relay position, of every space: the relay's epoch
    /// changed, so no position it gave holds any more (RFC 0007, "Restores
    /// and the relay epoch"). Refusals stay.
    pub fn forget_relay_positions(&mut self) -> Result<(), Error> {
        self.conn
            .execute("UPDATE log SET relay_position = NULL", [])?;
        for log in self.logs.values_mut() {
            log.forget_relay_confirmations();
        }
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

/// The `meta` key of the relay epoch, as 32 lowercase hex digits.
const RELAY_EPOCH: &str = "relay_epoch";
