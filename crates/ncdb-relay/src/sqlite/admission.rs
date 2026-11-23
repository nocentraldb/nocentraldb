//! The SQLite backend's admissions (RFC 0015, "How the relay takes it";
//! NOC-469): one row of `admissions` per identity, with the latest
//! admission's until, spaces and key, and the count of spaces the identity
//! created under its admissions. A later `admit` replaces the first three
//! and keeps the count; an append raises the count in its own transaction.

use rusqlite::{Connection, OptionalExtension, params};

use super::{array, failed, int, unsigned};
use crate::store::{AdmissionRecord, Admitted, StoreError};

/// Records `admitted` for `identity`, keeping the count of an earlier row.
pub(super) fn admit(
    connection: &Connection,
    identity: [u8; 32],
    admitted: Admitted,
) -> Result<(), StoreError> {
    connection
        .execute(
            "INSERT INTO admissions (identity, until, spaces, key, created)
             VALUES (?1, ?2, ?3, ?4, 0)
             ON CONFLICT (identity) DO UPDATE SET
                 until = excluded.until, spaces = excluded.spaces, key = excluded.key",
            params![
                identity,
                int(admitted.until),
                int(admitted.spaces),
                admitted.key
            ],
        )
        .map_err(failed("admit"))?;
    Ok(())
}

/// The identity's row, if `admit` wrote one.
pub(super) fn admission(
    connection: &Connection,
    identity: [u8; 32],
) -> Result<Option<AdmissionRecord>, StoreError> {
    let row: Option<(i64, i64, Vec<u8>, i64)> = connection
        .query_row(
            "SELECT until, spaces, key, created FROM admissions WHERE identity = ?1",
            [identity],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(failed("admission"))?;
    row.map(|(until, spaces, key, created)| {
        Ok(AdmissionRecord {
            until: unsigned(until),
            spaces: unsigned(spaces),
            key: array(key)?,
            created: unsigned(created),
        })
    })
    .transpose()
}

/// Whether the identity may create one more space under its admission:
/// it has one, and its count is below its spaces. Read inside the append's
/// write transaction, so no other append moves the count in between.
pub(super) fn holds(connection: &Connection, identity: [u8; 32]) -> Result<bool, StoreError> {
    Ok(admission(connection, identity)?.is_some_and(|record| record.created < record.spaces))
}

/// Raises the identity's count by one.
pub(super) fn count(connection: &Connection, identity: [u8; 32]) -> Result<(), StoreError> {
    connection
        .execute(
            "UPDATE admissions SET created = created + 1 WHERE identity = ?1",
            [identity],
        )
        .map_err(failed("admitted space"))?;
    Ok(())
}
