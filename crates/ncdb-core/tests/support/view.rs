//! Test support for the local view on SQLite (NOC-69, RFC 0006): a
//! minimal model, a fresh database file per test, and the in-memory fold
//! the SQLite view must agree with.
//!
//! Each test file includes this module with `#[path]`; not every file uses
//! every helper.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ncdb_core::basic::{self, BasicValue, Kind};
use ncdb_core::change::Operation;
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::ObjectId;
use ncdb_core::link::Link;
use ncdb_core::log::OperationParkReason;
use ncdb_core::object::{Object, ObjectOperation};
use ncdb_core::receive::Fold;
use ncdb_core::set::{self, Set};
use ncdb_core::view::{Direction, Field, FieldState, Index, Model};

/// The collection with fields, as `member.rs` names it.
pub const TASK: &str = "task";
/// A collection without fields.
pub const PROJECT: &str = "project";
pub const TITLE: &str = "title";
pub const RANK: &str = "rank";
pub const TAGS: &str = "tags";
pub const OWNER: &str = "owner";
/// An index of tasks by rank, with no equality prefix.
pub const BY_RANK: &str = "by_rank";
/// An index of tasks by title, then by rank, descending; a query fixes the
/// title.
pub const BY_TITLE: &str = "by_title";

/// The model of the tests: tasks with a text title, an integer rank, a set
/// of text tags and a link to a project; projects without fields; two
/// declared indexes.
pub fn model() -> Model {
    let text = || BasicValue { kind: Kind::Text };
    let mut task = BTreeMap::new();
    task.insert(TITLE.to_owned(), Field::Basic(text()));
    task.insert(
        RANK.to_owned(),
        Field::Basic(BasicValue {
            kind: Kind::Integer,
        }),
    );
    task.insert(TAGS.to_owned(), Field::Set(Set { kind: Kind::Text }));
    task.insert(
        OWNER.to_owned(),
        Field::Link(Link {
            collections: [PROJECT.to_owned()].into(),
        }),
    );
    let mut collections = BTreeMap::new();
    collections.insert(TASK.to_owned(), task);
    collections.insert(PROJECT.to_owned(), BTreeMap::new());
    Model {
        collections,
        indexes: vec![
            Index {
                name: BY_RANK.to_owned(),
                collection: TASK.to_owned(),
                keys: vec![(RANK.to_owned(), Direction::Ascending)],
                equality: 0,
            },
            Index {
                name: BY_TITLE.to_owned(),
                collection: TASK.to_owned(),
                keys: vec![
                    (TITLE.to_owned(), Direction::Ascending),
                    (RANK.to_owned(), Direction::Descending),
                ],
                equality: 1,
            },
        ],
    }
}

/// A path for a database file no other test uses. The directory is the
/// system's temporary one; the file is removed when the value drops.
pub struct TempDb(pub PathBuf);

impl TempDb {
    pub fn new(name: &str) -> TempDb {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let file = format!("ncdb-view-{}-{name}-{n}.sqlite", std::process::id());
        let path = std::env::temp_dir().join(file);
        let db = TempDb(path);
        db.remove();
        db
    }

    fn remove(&self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        self.remove();
    }
}

/// The in-memory fold of a model: every field's state and every object, as
/// the field types fold them. The SQLite view must give the same results.
#[derive(Clone, Debug)]
pub struct Reference {
    pub model: Model,
    pub fields: BTreeMap<(ObjectId, String, String), FieldState>,
    pub objects: BTreeMap<ObjectId, Object>,
}

impl Reference {
    pub fn new(model: Model) -> Reference {
        Reference {
            model,
            fields: BTreeMap::new(),
            objects: BTreeMap::new(),
        }
    }

    fn field(&self, collection: &str, field: &str) -> Option<&Field> {
        self.model.collections.get(collection)?.get(field)
    }
}

/// Decodes `operation` with `field_type` and applies it to `state`.
fn apply<T: Encoding>(
    field_type: &T,
    state: &mut T::State,
    operation: &Operation,
    origin: Origin,
) -> Result<(), OperationParkReason> {
    let op = field_type.decode(operation, origin)?;
    field_type.apply(state, &op);
    Ok(())
}

impl Fold for Reference {
    fn knows_collection(&self, collection: &str) -> bool {
        self.model.collections.contains_key(collection)
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.field(collection, field).is_some()
    }

    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        let name = operation.field.clone().expect("a field operation");
        let field = self
            .field(&operation.collection, &name)
            .expect("a known field")
            .clone();
        let key = (operation.object, operation.collection.clone(), name);
        let empty = match field {
            Field::Basic(_) | Field::Link(_) => FieldState::Basic(basic::State::default()),
            Field::Set(_) | Field::Links(_) => FieldState::Set(set::State::default()),
        };
        let mut state = self.fields.get(&key).cloned().unwrap_or(empty);
        match (&field, &mut state) {
            (Field::Basic(t), FieldState::Basic(s)) => apply(t, s, operation, origin)?,
            (Field::Link(t), FieldState::Basic(s)) => apply(t, s, operation, origin)?,
            (Field::Set(t), FieldState::Set(s)) => apply(t, s, operation, origin)?,
            (Field::Links(t), FieldState::Set(s)) => apply(t, s, operation, origin)?,
            _ => unreachable!("the state matches the field type"),
        }
        self.fields.insert(key, state);
        Ok(())
    }

    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        self.objects.entry(object).or_default().apply(operation);
    }
}

/// The names of the tables and indexes of a database file, by kind.
pub fn schema(conn: &rusqlite::Connection) -> BTreeSet<(String, String)> {
    let mut statement = conn
        .prepare("SELECT type, name FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// A database in memory that a second connection of this process can
/// open: the view's and the test's.
pub struct MemDb(pub PathBuf);

impl MemDb {
    pub fn new() -> MemDb {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        MemDb(PathBuf::from(format!(
            "file:ncdb-view-{n}?mode=memory&cache=shared"
        )))
    }

    pub fn connect(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.0).unwrap()
    }
}

/// The rows of every view table (`objects`, `c_*`, `fs_*`), in primary-key
/// order, as bytes: equal tables, equal bytes. Not the log, whose order is
/// the order of arrival.
pub fn view_bytes(conn: &rusqlite::Connection) -> Vec<u8> {
    use rusqlite::types::Value as Sql;
    let mut tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table'
             AND (name = 'objects' OR name LIKE 'c\\_%' ESCAPE '\\' OR name LIKE 'fs\\_%' ESCAPE '\\')",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    tables.sort();
    let mut bytes = Vec::new();
    for table in tables {
        bytes.extend(table.as_bytes());
        let sql = format!("SELECT * FROM \"{table}\" ORDER BY 1, 2, 3");
        let sql = if table.starts_with("fs_") {
            sql
        } else {
            format!("SELECT * FROM \"{table}\" ORDER BY 1, 2")
        };
        let mut statement = conn.prepare(&sql).unwrap();
        let columns = statement.column_count();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            bytes.push(b'\n');
            for i in 0..columns {
                match row.get::<_, Sql>(i).unwrap() {
                    Sql::Null => bytes.push(0),
                    Sql::Integer(n) => {
                        bytes.push(1);
                        bytes.extend(n.to_le_bytes());
                    }
                    Sql::Real(x) => {
                        bytes.push(2);
                        bytes.extend(x.to_bits().to_le_bytes());
                    }
                    Sql::Text(t) => {
                        bytes.push(3);
                        bytes.extend((t.len() as u64).to_le_bytes());
                        bytes.extend(t.as_bytes());
                    }
                    Sql::Blob(b) => {
                        bytes.push(4);
                        bytes.extend((b.len() as u64).to_le_bytes());
                        bytes.extend(b);
                    }
                }
            }
        }
    }
    bytes
}

impl Reference {
    /// What the view reads of `object` in `collection`: its object and the
    /// fields of the collection that were written, or `None` when the copy
    /// holds no operation on either.
    pub fn row(&self, collection: &str, object: ObjectId) -> Option<ncdb_core::view::Row> {
        let fields: BTreeMap<String, FieldState> = self
            .fields
            .iter()
            .filter(|((o, c, _), _)| *o == object && c == collection)
            .map(|((_, _, f), state)| (f.clone(), state.clone()))
            .collect();
        let known = self.objects.get(&object);
        if known.is_none() && fields.is_empty() {
            return None;
        }
        Some(ncdb_core::view::Row {
            object: known.cloned().unwrap_or_default(),
            fields,
        })
    }
}
