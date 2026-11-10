//! The tables of the view (RFC 0006, "Where the view lives") and the
//! version in `meta` that says whether they match the code and the model.
//! The layout is internal to the core and not a stable format (RFC 0006,
//! decision 2); only the `log` table follows RFC 0002, "The log on disk".

use rusqlite::{Connection, OptionalExtension, params};

use super::model::{Direction, Field, Index, Model, Scope};
use super::{Error, stored_lock};
use crate::basic::Kind;
use crate::model_lock::ModelLock;

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

/// The columns of a declared index's SQLite index, in order, each with
/// its direction (RFC 0006, "Declared indexes"). Every index ends with the
/// object id, so its order is total: one that spans every space orders by
/// its keys, then the space id; one that runs in one space puts the space
/// id first (RFC 0006, "Declared indexes").
pub(super) fn index_columns(index: &Index) -> Vec<(String, Direction)> {
    let ascending = |name: &str| (name.to_owned(), Direction::Ascending);
    let keys = index
        .keys
        .iter()
        .map(|(field, direction)| (column(field), *direction));
    match index.scope {
        Scope::Member => keys
            .chain([ascending("space_id"), ascending("object_id")])
            .collect(),
        Scope::Space => std::iter::once(ascending("space_id"))
            .chain(keys)
            .chain([ascending("object_id")])
            .collect(),
    }
}

/// How many leading columns of an index's SQLite index a lookup fixes with
/// equality: the space id when the index runs in one space, then the
/// declared equality prefix.
pub(super) fn prefix(index: &Index) -> usize {
    index.equality + usize::from(index.scope == Scope::Space)
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

/// Whether the view's tables match the model the view is opened with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Fit {
    /// They do: new tables, or tables folded by this model.
    Current,
    /// They were folded by another model, which the lock check accepted:
    /// the log must be folded again (RFC 0006, "Rebuilding the view").
    Refold,
}

/// A value of `meta`, when it holds one.
pub(super) fn meta(conn: &Connection, key: &str) -> Result<Option<String>, Error> {
    let value = conn
        .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()?;
    Ok(value)
}

/// Opens the view's tables on `conn`: creates them in a new database, and
/// otherwise checks that `meta` names this format, and `lock` against the
/// lock the database was last opened with (`super::stored_lock`). Says
/// whether the tables were folded by `model`.
pub(super) fn open(conn: &Connection, model: &Model, lock: &ModelLock) -> Result<Fit, Error> {
    conn.execute_batch(FIXED)?;
    let hash = hex(&model.hash());
    match meta(conn, "format")? {
        None => {
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('format', ?1), ('model', ?2)",
                params![super::View::FORMAT.to_string(), hash],
            )?;
            stored_lock::store(conn, lock)?;
            create(conn, model)?;
            Ok(Fit::Current)
        }
        Some(format) => {
            let found = format.parse().map_err(|_| Error::Corrupt("meta format"))?;
            if found != super::View::FORMAT {
                return Err(Error::Format { found });
            }
            let current = meta(conn, "model")?.as_deref() == Some(hash.as_str());
            stored_lock::check(conn, lock, current)?;
            Ok(match current {
                true => Fit::Current,
                false => Fit::Refold,
            })
        }
    }
}

/// Drops the tables of every collection, with their indexes, and the
/// objects' states, and creates the tables of `model` in their place,
/// empty: the start of folding the log again. `meta` then names `model`.
pub(super) fn replace(conn: &Connection, model: &Model) -> Result<(), Error> {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table'
             AND (name LIKE 'c\\_%' ESCAPE '\\' OR name LIKE 'fs\\_%' ESCAPE '\\')",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for table in tables {
        conn.execute_batch(&format!("DROP TABLE {};", quote(&table)))?;
    }
    conn.execute("DELETE FROM objects", [])?;
    create(conn, model)?;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'model'",
        [hex(&model.hash())],
    )?;
    Ok(())
}

/// Creates the tables of every collection and the SQLite index of every
/// declared index. A collection's table holds, beside the columns of its
/// fields, whether the object is deleted, so that its indexes leave the
/// deleted objects out while a read by id still finds them.
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
                object_id BLOB NOT NULL,
                deleted INTEGER NOT NULL DEFAULT 0{columns},
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
        let keys: Vec<String> = index_columns(index)
            .into_iter()
            .map(|(name, direction)| match direction {
                Direction::Ascending => format!("{name} ASC"),
                Direction::Descending => format!("{name} DESC"),
            })
            .collect();
        // A partial index: no declared index holds a deleted object
        // (RFC 0006, "Declared indexes"), and a lookup names the same condition.
        conn.execute_batch(&format!(
            "CREATE INDEX {} ON {} ({}) WHERE deleted = 0;",
            quote(&index_name(index)),
            rows(&index.collection),
            keys.join(", "),
        ))?;
    }
    Ok(())
}

/// Bytes as lower-case hex.
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
