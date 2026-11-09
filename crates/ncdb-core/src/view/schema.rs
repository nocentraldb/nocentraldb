//! The tables of the view (RFC 0006, "Where the view lives") and the
//! version in `meta` that says whether they match the code and the model.
//! The layout is internal to the core and not a stable format (RFC 0006,
//! decision 2); only the `log` table follows RFC 0002, "The log on disk".

use rusqlite::{Connection, OptionalExtension, params};

use super::Error;
use super::model::{Direction, Field, Index, Model};
use crate::basic::Kind;

/// The tables every view has, whatever its model.
const FIXED: &str = "
CREATE TABLE IF NOT EXISTS log (
    local_position INTEGER PRIMARY KEY,
    space_id BLOB NOT NULL,
    change_id BLOB NOT NULL,
    relay_position INTEGER,
    relay_refusal TEXT,
    state TEXT NOT NULL,
    reason TEXT,
    frame BLOB NOT NULL,
    UNIQUE (space_id, change_id)
) STRICT;
CREATE TABLE IF NOT EXISTS parked (
    space_id BLOB NOT NULL,
    change_id BLOB NOT NULL,
    position INTEGER NOT NULL,
    reason TEXT NOT NULL,
    PRIMARY KEY (space_id, change_id, position)
) STRICT;
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS objects (
    space_id BLOB NOT NULL,
    object_id BLOB NOT NULL,
    state BLOB NOT NULL,
    PRIMARY KEY (space_id, object_id)
) STRICT;
";

/// An SQL identifier, quoted, so that any collection or field name is one.
pub(super) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// The table of a collection's read values.
pub(super) fn rows(collection: &str) -> String {
    quote(&format!("c_{collection}"))
}

/// The table of a collection's merge states.
pub(super) fn states(collection: &str) -> String {
    quote(&format!("fs_{collection}"))
}

/// The column of a field's read value.
pub(super) fn column(field: &str) -> String {
    quote(&format!("f_{field}"))
}

/// The SQLite index of a declared index, qualified by its collection,
/// since index names are scoped per collection. A name of the lock has no
/// `_` (RFC 0005), so no two declared indexes share an SQLite index.
pub(super) fn index_name(index: &Index) -> String {
    format!("ix_{}_{}", index.collection, index.name)
}

/// The SQLite type a field's column holds (RFC 0006: `INTEGER`, `REAL`,
/// `TEXT` or `BLOB`).
fn column_type(field: &Field) -> &'static str {
    match field {
        Field::Basic(t) => match t.kind {
            Kind::Text | Kind::Choice(_) => "TEXT",
            Kind::Integer | Kind::Boolean => "INTEGER",
            Kind::Float => "REAL",
        },
        _ => "BLOB",
    }
}

/// Opens the view's tables on `conn`: creates them in a new database, and
/// otherwise checks that `meta` names this format and this model.
pub(super) fn open(conn: &Connection, model: &Model) -> Result<(), Error> {
    conn.execute_batch(FIXED)?;
    let hash = hex(&model.hash());
    let meta = |key: &str| -> Result<Option<String>, Error> {
        let value = conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?;
        Ok(value)
    };
    match meta("format")? {
        None => {
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('format', ?1), ('model', ?2)",
                params![super::View::FORMAT.to_string(), hash],
            )?;
            create(conn, model)
        }
        Some(format) => {
            let found = format.parse().map_err(|_| Error::Corrupt("meta format"))?;
            if found != super::View::FORMAT {
                return Err(Error::Format { found });
            }
            if meta("model")?.as_deref() != Some(hash.as_str()) {
                return Err(Error::Model);
            }
            Ok(())
        }
    }
}

/// Creates the tables of every collection and the SQLite index of every
/// declared index.
fn create(conn: &Connection, model: &Model) -> Result<(), Error> {
    for (collection, fields) in &model.collections {
        let columns: String = fields
            .iter()
            .filter(|(_, field)| field.has_column())
            .map(|(name, field)| format!(", {} {}", column(name), column_type(field)))
            .collect();
        conn.execute_batch(&format!(
            "CREATE TABLE {} (
                space_id BLOB NOT NULL,
                object_id BLOB NOT NULL{columns},
                PRIMARY KEY (space_id, object_id)
            ) STRICT;
            CREATE TABLE {} (
                space_id BLOB NOT NULL,
                object_id BLOB NOT NULL,
                field TEXT NOT NULL,
                state BLOB NOT NULL,
                PRIMARY KEY (space_id, object_id, field)
            ) STRICT;",
            rows(collection),
            states(collection),
        ))?;
    }
    for index in &model.indexes {
        // Every index ends with (space_id, object_id), so its order is
        // total (RFC 0006, "Declared indexes").
        let keys: String = index
            .keys
            .iter()
            .map(|(field, direction)| {
                let direction = match direction {
                    Direction::Ascending => "ASC",
                    Direction::Descending => "DESC",
                };
                format!("{} {direction}, ", column(field))
            })
            .collect();
        conn.execute_batch(&format!(
            "CREATE INDEX {} ON {} ({keys}space_id, object_id);",
            quote(&index_name(index)),
            rows(&index.collection),
        ))?;
    }
    Ok(())
}

/// Bytes as lower-case hex.
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
