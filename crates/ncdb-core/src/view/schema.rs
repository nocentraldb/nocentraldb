//! The tables of the view (RFC 0006, "Where the view lives") and the
//! version in `meta` that says whether they match the code and the model.
//! The layout is internal to the core and not a stable format (RFC 0006,
//! decision 2); only the `log` table follows RFC 0002, "The log on disk".
//! `confirmed_accounts` is no part of the view: it holds what a person
//! confirmed, which no fold computes (`super::accounts`). The `log`'s
//! `written_at` came later (NOC-231): a database from before gains it on
//! open, `NULL` in every row, and keeps its format version. Nor is
//! `shreds_held`: when this device first held each shred
//! (`super::shreds_held`).

use crate::plugins::Registry;
use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, params};

pub(super) use super::migrate::Additions;
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
    written_at INTEGER,
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
CREATE TABLE IF NOT EXISTS confirmed_accounts (
    account_key BLOB PRIMARY KEY NOT NULL,
    confirmed_at INTEGER NOT NULL
) STRICT;
CREATE TABLE IF NOT EXISTS pending_links (
    statement BLOB PRIMARY KEY NOT NULL,
    change_id BLOB
) STRICT;
CREATE TABLE IF NOT EXISTS joined (
    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1)
) STRICT;
CREATE TABLE IF NOT EXISTS shreds_held (
    space_id BLOB NOT NULL,
    change_id BLOB NOT NULL,
    held_at INTEGER NOT NULL,
    PRIMARY KEY (space_id, change_id)
) STRICT;
CREATE TABLE IF NOT EXISTS objects (
    space_id BLOB NOT NULL,
    object_id BLOB NOT NULL,
    state BLOB NOT NULL,
    PRIMARY KEY (space_id, object_id)
) STRICT;
";

/// Adds `written_at` to the `log` of a database from before it (NOC-231):
/// additive, so its rows read `NULL` and a core that does not know the
/// column ignores it.
fn add_written_at(conn: &Connection) -> Result<(), Error> {
    let has: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info('log') WHERE name = 'written_at')",
        [],
        |row| row.get(0),
    )?;
    if !has {
        conn.execute_batch("ALTER TABLE log ADD COLUMN written_at INTEGER;")?;
    }
    Ok(())
}

/// An SQL identifier, quoted, so that any collection or field name is one.
pub(super) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// The table of a collection's read values.
pub(super) fn rows(collection: &str) -> String {
    rows_in("", collection)
}

/// The table of a collection's merge states.
pub(super) fn states(collection: &str) -> String {
    states_in("", collection)
}

/// The prefix of the tables a full rebuild folds into, beside the view's
/// own, until it swaps them in (RFC 0006, "How"; NOC-227).
pub(super) const STAGING: &str = "r_";

/// The table of a collection's read values, under `prefix`: empty for the
/// view's own tables, [`STAGING`] for a rebuild's.
pub(super) fn rows_in(prefix: &str, collection: &str) -> String {
    quote(&format!("{prefix}c_{collection}"))
}

/// The table of a collection's merge states, under `prefix`.
pub(super) fn states_in(prefix: &str, collection: &str) -> String {
    quote(&format!("{prefix}fs_{collection}"))
}

/// The table of the objects' creates and deletes, under `prefix`.
pub(super) fn objects_in(prefix: &str) -> String {
    quote(&format!("{prefix}objects"))
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fit {
    /// They do: new tables, or tables folded by this model.
    Current,
    /// They were folded by the model of the stored lock, and the model
    /// they are opened with only adds to it: the view is migrated in place
    /// (RFC 0006, "Rebuilding the view"; `super::migrate`).
    Migrate(Additions),
    /// They were folded by another model, which the lock check accepted:
    /// the log must be folded again (RFC 0006, "Rebuilding the view").
    /// Holds that model when it is known, the stored lock's, so that the
    /// old tables can serve while a rebuild runs (`super::rebuild`).
    Refold(Option<Box<Model>>),
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
pub(super) fn open(
    conn: &Connection,
    model: &Model,
    lock: &ModelLock,
    registry: &Registry,
) -> Result<Fit, Error> {
    conn.execute_batch(FIXED)?;
    add_written_at(conn)?;
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
            let folded_by = meta(conn, "model")?;
            let current = folded_by.as_deref() == Some(hash.as_str());
            // The lock the tables may have been folded by, read before the
            // check stores the new one in its place.
            let stored = meta(conn, "lock")?;
            stored_lock::check(conn, lock, current)?;
            if current {
                return Ok(Fit::Current);
            }
            // The model the tables were folded by: the stored lock's, with
            // the plugins of this open or with none, so that a plugin
            // registered now finds the fields it adds (RFC 0011, "A type
            // that is not registered"; NOC-84).
            let stored = stored.and_then(|text| ModelLock::from_json(&text).ok());
            let old = [registry, &Registry::default()]
                .into_iter()
                .filter_map(|registry| Model::from_lock_with(stored.as_ref()?, registry).ok())
                .find(|old| folded_by.as_deref() == Some(hex(&old.hash()).as_str()));
            Ok(match old {
                Some(old) => match Additions::between(&old, model) {
                    Some(additions) => Fit::Migrate(additions),
                    None => Fit::Refold(Some(Box::new(old))),
                },
                None => Fit::Refold(None),
            })
        }
    }
}

/// Drops the tables of every collection, with their indexes, and the
/// objects' states, and creates the tables of `model` in their place,
/// empty: the start of folding the log again. `meta` then names `model`.
pub(super) fn replace(conn: &Connection, model: &Model) -> Result<(), Error> {
    drop_collections(conn)?;
    conn.execute("DELETE FROM objects", [])?;
    create(conn, model)?;
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'model'",
        [hex(&model.hash())],
    )?;
    Ok(())
}

/// Drops the tables of every collection, with their indexes.
fn drop_collections(conn: &Connection) -> Result<(), Error> {
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
    Ok(())
}

/// Creates the tables of every collection and the SQLite index of every
/// declared index. A collection's table holds, beside the columns of its
/// fields, whether the object is deleted, so that its indexes leave the
/// deleted objects out while a read by id still finds them.
fn create(conn: &Connection, model: &Model) -> Result<(), Error> {
    for (collection, fields) in &model.collections {
        create_collection(conn, collection, fields)?;
    }
    for index in &model.indexes {
        create_index(conn, index)?;
    }
    Ok(())
}

/// Creates the tables of one collection: its read values, with a column
/// for each field that has one, and its merge states.
pub(super) fn create_collection(
    conn: &Connection,
    collection: &str,
    fields: &BTreeMap<String, Field>,
) -> Result<(), Error> {
    create_collection_in(conn, "", collection, fields)
}

/// [`create_collection`], under `prefix`.
pub(super) fn create_collection_in(
    conn: &Connection,
    prefix: &str,
    collection: &str,
    fields: &BTreeMap<String, Field>,
) -> Result<(), Error> {
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
        rows_in(prefix, collection),
        states_in(prefix, collection),
    ))?;
    Ok(())
}

/// Drops the staging tables a rebuild cut off by a close left behind.
pub(super) fn drop_staging(conn: &Connection) -> Result<(), Error> {
    let tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table'
             AND name LIKE 'r\\_%' ESCAPE '\\'",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for table in tables {
        conn.execute_batch(&format!("DROP TABLE {};", quote(&table)))?;
    }
    Ok(())
}

/// Creates a rebuild's staging tables for `model`, empty and without
/// indexes, which the swap creates once the tables hold everything.
pub(super) fn create_staging(conn: &Connection, model: &Model) -> Result<(), Error> {
    for (collection, fields) in &model.collections {
        create_collection_in(conn, STAGING, collection, fields)?;
    }
    conn.execute_batch(&format!(
        "CREATE TABLE {} (
            space_id BLOB NOT NULL,
            object_id BLOB NOT NULL,
            state BLOB NOT NULL,
            PRIMARY KEY (space_id, object_id)
        ) STRICT;",
        objects_in(STAGING)
    ))?;
    Ok(())
}

/// Swaps a rebuild's staging tables in: drops the view's collection tables
/// and objects, renames the staging tables in their place, creates the
/// indexes of `model` and names it in `meta`.
pub(super) fn swap_staging(conn: &Connection, model: &Model) -> Result<(), Error> {
    drop_collections(conn)?;
    conn.execute_batch("DROP TABLE objects;")?;
    for collection in model.collections.keys() {
        conn.execute_batch(&format!(
            "ALTER TABLE {} RENAME TO {};
             ALTER TABLE {} RENAME TO {};",
            rows_in(STAGING, collection),
            rows(collection),
            states_in(STAGING, collection),
            states(collection),
        ))?;
    }
    conn.execute_batch(&format!(
        "ALTER TABLE {} RENAME TO objects;",
        objects_in(STAGING)
    ))?;
    for index in &model.indexes {
        create_index(conn, index)?;
    }
    conn.execute(
        "UPDATE meta SET value = ?1 WHERE key = 'model'",
        [hex(&model.hash())],
    )?;
    Ok(())
}

/// Adds the column of a new field to its collection's table, when the
/// field has one; existing rows read it as `NULL` until an operation on
/// it is folded.
pub(super) fn add_column(
    conn: &Connection,
    collection: &str,
    name: &str,
    field: &Field,
) -> Result<(), Error> {
    if field.has_column() {
        conn.execute_batch(&format!(
            "ALTER TABLE {} ADD COLUMN {} {};",
            rows(collection),
            column(name),
            column_type(field),
        ))?;
    }
    Ok(())
}

/// Creates the SQLite index of a declared index.
pub(super) fn create_index(conn: &Connection, index: &Index) -> Result<(), Error> {
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
    Ok(())
}

/// Bytes as lower-case hex.
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
