//! The column values the fold writes into `c_<collection>` (RFC 0006,
//! "Where the view lives"): a basic value's or a link's winning write, a
//! personal field's value opened with its person key, and a reference.

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params};

use super::Error;
use super::fold::PersonKeys;
use super::model::{Field, Model};
use super::schema::{column, rows_in, states_in};
use super::state;
use crate::basic::{BasicValue, Entry};
use crate::cbor::Value;
use crate::field::FieldType;
use crate::ids::{ObjectId, SpaceId};
use crate::personal::{self, Place, Sealed};
use crate::reference;
use crate::texts;

/// The column value of a basic value's or a link's winning write: its
/// value as the SQLite type the field names, or NULL when no write is
/// live, the field is cleared, or the value is outside the reader's
/// schema (it stays readable in the merge state).
pub(super) fn column_value(winner: Option<Entry>) -> Sql {
    match winner.filter(|entry| entry.known) {
        Some(entry) => sql_value(entry.value),
        None => Sql::Null,
    }
}

/// The column value of a personal field's winning write (RFC 0006, "What
/// the view stores in plaintext"): its sealed value opened with the person
/// key it names, at `place`, when `keys` hold that key and the value fits
/// the field's kind. NULL otherwise: no live write, a clear, a key not
/// held, an erased person, a value that does not open, or one sealed
/// under a person key that lost (RFC 0009, "How a personal field reads").
/// `place.person_key` is ignored; the sealed value names it.
pub(super) fn personal_column(
    field_type: &BasicValue,
    mut place: Place<'_>,
    winner: Option<Entry>,
    keys: &PersonKeys,
) -> Sql {
    let Some(sealed) = winner.and_then(|entry| Sealed::from_value(&entry.value)) else {
        return Sql::Null;
    };
    let Some(key) = keys.get(&sealed.person_key) else {
        return Sql::Null;
    };
    place.person_key = sealed.person_key;
    match personal::open(key, &place, &sealed.bytes) {
        Ok(value) if field_type.kind.allows(&value) => sql_value(value),
        _ => Sql::Null,
    }
}

/// Opens every personal field of `space` again with `keys`, from the merge
/// state each keeps sealed, into its column of `c_<collection>`: after the
/// keys the view holds for the space changed.
pub(super) fn open_personal(
    conn: &Connection,
    model: &Model,
    space: SpaceId,
    keys: &PersonKeys,
) -> Result<(), Error> {
    for (collection, fields) in &model.personal {
        for name in fields.keys() {
            let Some(Field::Basic(field_type)) = model.field(collection, name) else {
                continue;
            };
            let mut select = conn.prepare(&format!(
                "SELECT object_id, state FROM {} WHERE space_id = ?1 AND field = ?2",
                states_in("", collection)
            ))?;
            let rows = select
                .query_map(params![space.0, name], |r| {
                    Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (object, bytes) in rows {
                let object = ObjectId(
                    object
                        .try_into()
                        .map_err(|_| Error::Corrupt(texts::UNREADABLE_OBJECT_ID))?,
                );
                let winner = field_type.read(&state::basic(&bytes)?).value;
                let place = Place {
                    space,
                    object,
                    person_key: [0; 16],
                    collection,
                    field: name,
                };
                conn.execute(
                    &format!(
                        "UPDATE {} SET {} = ?3 WHERE space_id = ?1 AND object_id = ?2",
                        rows_in("", collection),
                        column(name)
                    ),
                    params![
                        space.0,
                        object.0,
                        personal_column(field_type, place, winner, keys)
                    ],
                )?;
            }
        }
    }
    Ok(())
}

/// The column value of a reference's winning write: the target's space id
/// followed by its object id, 32 bytes, as stored and whatever its state
/// (RFC 0006, "Declared indexes"). NULL when no write is live, the reference is
/// cleared, or its value does not fit (RFC 0004, "The value and its
/// encoding").
pub(super) fn reference_column(winner: Option<Entry>) -> Sql {
    winner.map_or(Sql::Null, |entry| reference_sql(&entry.value))
}

/// A reference's value as the SQLite value its column holds: 32 bytes, or
/// NULL for a value that names no target.
pub(super) fn reference_sql(value: &Value) -> Sql {
    match reference::target(value) {
        Some(target) => Sql::Blob([target.space.0, target.object.0].concat()),
        None => Sql::Null,
    }
}

/// A basic value as the SQLite value its column holds: text as `TEXT`,
/// an integer and a boolean as `INTEGER`, a float as `REAL`, bytes (an
/// object id) as `BLOB`, anything else as NULL.
pub(super) fn sql_value(value: Value) -> Sql {
    match value {
        Value::Text(text) => Sql::Text(text),
        Value::Unsigned(n) => i64::try_from(n).map_or(Sql::Null, Sql::Integer),
        Value::Negative(n) => i64::try_from(n).map_or(Sql::Null, |n| Sql::Integer(-1 - n)),
        Value::Float(x) => Sql::Real(x),
        Value::Bool(b) => Sql::Integer(i64::from(b)),
        Value::Bytes(bytes) => Sql::Blob(bytes),
        Value::Null | Value::Array(_) | Value::Map(_) => Sql::Null,
    }
}
