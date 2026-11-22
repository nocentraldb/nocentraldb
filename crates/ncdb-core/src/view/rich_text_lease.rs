//! The device-local table of rich-text client ids (RFC 0013, "Reusing a
//! client id"): for each field of an object, the client ids this member
//! used, the nonce of each, the clock it reached, and whether a writer
//! holds it now. A binding, or a write by an agent, leases one for as long
//! as it writes, so a document collects one client id per device and
//! parallel writer instead of one per editor session, and a member never
//! writes one item id twice.
//!
//! The table `rich_text_clients` lives in the member's SQLite file beside
//! the view, but is no part of it: no fold computes it, so neither folding
//! the log again nor a rebuild touches it. It is made when first used
//! (`CREATE TABLE IF NOT EXISTS`), and a core that does not know it
//! ignores it, so the view format version stays as it is.

use rusqlite::{OptionalExtension, params};

use super::{Error, View};
use crate::ids::{MemberId, ObjectId, SpaceId};
use crate::rich_text::client_id;

/// A client id leased to one writer of one rich-text field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    /// The space of the object.
    pub space: SpaceId,
    /// The object.
    pub object: ObjectId,
    /// The Yjs client id, derived from the member and the nonce.
    pub client: u64,
    /// The nonce an operation 5 lists for the client id.
    pub nonce: [u8; 8],
    /// The clock the client id reached when it was last released.
    pub clock: u64,
}

const TABLE: &str = "CREATE TABLE IF NOT EXISTS rich_text_clients (
    space_id BLOB NOT NULL,
    object_id BLOB NOT NULL,
    collection TEXT NOT NULL,
    field TEXT NOT NULL,
    client_id INTEGER NOT NULL,
    nonce BLOB NOT NULL,
    clock INTEGER NOT NULL,
    leased INTEGER NOT NULL,
    PRIMARY KEY (space_id, object_id, collection, field, client_id)
)";

impl View {
    /// Leases a client id of `member` for `field` of `object` in
    /// `collection`: one no writer holds now, the lowest first, or a new
    /// one with a fresh nonce.
    pub fn lease_rich_text(
        &mut self,
        member: MemberId,
        space: SpaceId,
        collection: &str,
        object: ObjectId,
        field: &str,
    ) -> Result<Lease, Error> {
        self.conn.execute(TABLE, [])?;
        let tx = self.conn.transaction()?;
        let free: Option<(i64, Vec<u8>, i64)> = tx
            .query_row(
                "SELECT client_id, nonce, clock FROM rich_text_clients
                 WHERE space_id = ?1 AND object_id = ?2 AND collection = ?3 AND field = ?4
                   AND leased = 0
                 ORDER BY client_id LIMIT 1",
                params![space.0, object.0, collection, field],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let lease = match free {
            Some((client, nonce, clock)) => Lease {
                space,
                object,
                client: client as u64,
                nonce: nonce.try_into().map_err(|_| corrupt())?,
                clock: clock as u64,
            },
            None => {
                let mut nonce = [0u8; 8];
                getrandom::fill(&mut nonce).expect("the platform's secure random generator failed");
                Lease {
                    space,
                    object,
                    client: client_id(&member, &nonce),
                    nonce,
                    clock: 0,
                }
            }
        };
        tx.execute(
            "INSERT INTO rich_text_clients
                 (space_id, object_id, collection, field, client_id, nonce, clock, leased)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1)
             ON CONFLICT DO UPDATE SET leased = 1",
            params![
                space.0,
                object.0,
                collection,
                field,
                lease.client as i64,
                lease.nonce,
                lease.clock as i64
            ],
        )?;
        tx.commit()?;
        Ok(lease)
    }

    /// Gives `lease` back, with the clock its writer reached; the clock
    /// kept never moves back.
    pub fn release_rich_text(&mut self, lease: &Lease, clock: u64) -> Result<(), Error> {
        self.conn.execute(TABLE, [])?;
        self.conn.execute(
            "UPDATE rich_text_clients SET leased = 0, clock = MAX(clock, ?3)
             WHERE space_id = ?1 AND object_id = ?2 AND client_id = ?4",
            params![
                lease.space.0,
                lease.object.0,
                i64::try_from(clock).map_err(|_| corrupt())?,
                lease.client as i64
            ],
        )?;
        Ok(())
    }
}

fn corrupt() -> Error {
    Error::Corrupt(crate::texts::UNREADABLE_MERGE_STATE)
}
