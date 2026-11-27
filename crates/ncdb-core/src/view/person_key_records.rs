//! The person-key records this device holds the keys of (RFC 0003, "The
//! person key"; NOC-547). A member holds a person key from its signed
//! record, which wraps the key under one generation of the space key
//! (RFC 0007, "Keys and the person-key store"). The device keeps each
//! record it accepted, as the relay stores it, so that a reopened database
//! holds its person keys again from its own copy, with no relay: the
//! space keys come back from the envelopes in the log, and each record
//! unwraps under one of them. No person key is on the disk in the clear.
//!
//! The table `person_key_records` holds one row per space, person-key id,
//! generation and writer, with the wrapped key, the writer's signature
//! and whether the record waits to be put to a relay: a record this member
//! wrote waits until a relay took it, also across a reopen, so a record
//! written without a relay, or while it was out of reach, reaches the
//! relay once the device connects. Like `shreds_held`, it lives in the member's SQLite file
//! beside the view but is no part of it: no fold computes it, so neither
//! folding the log again nor a rebuild touches it. A shred removes the
//! records of the key it destroys (RFC 0008, "A shred is final when
//! accepted"), and `secure_delete` overwrites their bytes. A database from
//! before the table gains it, empty, the next time it opens (`CREATE TABLE
//! IF NOT EXISTS`), and a core that does not know it ignores it, so the
//! view format version stays as it is.

use rusqlite::params;

use super::{Error, View};
use crate::ids::SpaceId;
use crate::texts;

/// A person-key record as the device keeps it (RFC 0007, `record`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonKeyRecord {
    /// The person-key id.
    pub person_key: [u8; 16],
    /// The generation of the space key that wraps it.
    pub generation: u64,
    /// The member id of its writer.
    pub writer: [u8; 32],
    /// The person key, wrapped under the generation's key.
    pub wrapped: Vec<u8>,
    /// The writer's signature.
    pub signature: [u8; 64],
}

impl View {
    /// Keeps `record` of `space`, waiting to be put to a relay when
    /// `waiting`. A record kept before stays as it was, except that it no
    /// longer waits once it is kept as not waiting: a relay holds it.
    pub fn keep_person_key_record(
        &mut self,
        space: SpaceId,
        record: &PersonKeyRecord,
        waiting: bool,
    ) -> Result<(), Error> {
        let generation = i64::try_from(record.generation)
            .map_err(|_| Error::Corrupt(texts::UNREADABLE_PERSON_KEY_RECORD))?;
        self.conn.execute(
            "INSERT INTO person_key_records
                 (space_id, person_key_id, generation, writer, wrapped, signature, waiting)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT (space_id, person_key_id, generation, writer)
             DO UPDATE SET waiting = MIN(waiting, excluded.waiting)",
            params![
                space.0,
                record.person_key,
                generation,
                record.writer,
                record.wrapped,
                record.signature,
                waiting
            ],
        )?;
        Ok(())
    }

    /// The records that wait to be put to a relay, by space, in the order
    /// of their space, person-key id, generation and writer.
    pub fn waiting_person_key_records(&self) -> Result<Vec<(SpaceId, PersonKeyRecord)>, Error> {
        let mut select = self.conn.prepare(
            "SELECT space_id, person_key_id, generation, writer, wrapped, signature
             FROM person_key_records WHERE waiting = 1
             ORDER BY space_id, person_key_id, generation, writer",
        )?;
        let rows = select
            .query_map([], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    (
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Vec<u8>>(3)?,
                        row.get::<_, Vec<u8>>(4)?,
                        row.get::<_, Vec<u8>>(5)?,
                    ),
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(space, record)| {
                let space = space
                    .try_into()
                    .map_err(|_| Error::Corrupt(texts::UNREADABLE_PERSON_KEY_RECORD))?;
                Ok((SpaceId(space), read(record)?))
            })
            .collect()
    }

    /// Notes that a relay took `records` of `space`: they wait no longer.
    pub fn person_key_records_put(
        &mut self,
        space: SpaceId,
        records: &[PersonKeyRecord],
    ) -> Result<(), Error> {
        for record in records {
            let generation = i64::try_from(record.generation)
                .map_err(|_| Error::Corrupt(texts::UNREADABLE_PERSON_KEY_RECORD))?;
            self.conn.execute(
                "UPDATE person_key_records SET waiting = 0
                 WHERE space_id = ?1 AND person_key_id = ?2 AND generation = ?3 AND writer = ?4",
                params![space.0, record.person_key, generation, record.writer],
            )?;
        }
        Ok(())
    }

    /// The person-key records of `space` the device keeps, in the order of
    /// their person-key id, generation and writer.
    pub fn person_key_records(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, Error> {
        let mut select = self.conn.prepare(
            "SELECT person_key_id, generation, writer, wrapped, signature
             FROM person_key_records WHERE space_id = ?1
             ORDER BY person_key_id, generation, writer",
        )?;
        let rows = select
            .query_map(params![space.0], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter().map(read).collect()
    }

    /// Removes every record of the person key `person_key` of `space`: its
    /// key was destroyed.
    pub fn forget_person_key_records(
        &mut self,
        space: SpaceId,
        person_key: [u8; 16],
    ) -> Result<(), Error> {
        self.conn.execute(
            "DELETE FROM person_key_records WHERE space_id = ?1 AND person_key_id = ?2",
            params![space.0, person_key],
        )?;
        Ok(())
    }
}

/// A record from the columns of its row.
fn read(
    (person_key, generation, writer, wrapped, signature): (Vec<u8>, i64, Vec<u8>, Vec<u8>, Vec<u8>),
) -> Result<PersonKeyRecord, Error> {
    let unreadable = || Error::Corrupt(texts::UNREADABLE_PERSON_KEY_RECORD);
    Ok(PersonKeyRecord {
        person_key: person_key.try_into().map_err(|_| unreadable())?,
        generation: u64::try_from(generation).map_err(|_| unreadable())?,
        writer: writer.try_into().map_err(|_| unreadable())?,
        wrapped,
        signature: signature.try_into().map_err(|_| unreadable())?,
    })
}
