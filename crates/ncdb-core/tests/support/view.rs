//! Test support for the local view on SQLite (NOC-69, RFC 0006): a
//! minimal model read from its `model.lock.json` (NOC-77), a fresh
//! database file per test, and the in-memory fold the SQLite view must
//! agree with.
//!
//! Each test file includes this module with `#[path]`; not every file uses
//! every helper.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ncdb_core::change::Operation;
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::ObjectId;
use ncdb_core::log::OperationParkReason;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::object::{Object, ObjectOperation};
use ncdb_core::receive::Fold;
use ncdb_core::view::{Field, FieldState, Model};

/// The tasks, with fields of each kind, as `member.rs` names them.
pub const TASK: &str = "task";
/// Projects, with a name.
pub const PROJECT: &str = "project";
pub const TITLE: &str = "title";
pub const RANK: &str = "rank";
pub const TAGS: &str = "tags";
pub const OWNER: &str = "owner";
pub const NAME: &str = "name";
/// An index of tasks by rank, with no equality prefix.
pub const BY_RANK: &str = "byRank";
/// An index of tasks by title, then by rank, descending; a query fixes the
/// title.
pub const BY_TITLE: &str = "byTitle";
/// An index of tasks by title alone, and of projects by name alone, with
/// no equality prefix: each collection has its own index of this name.
pub const BY_NAME: &str = "byName";

/// The lock of the tests' model, as `ncdb model build` wrote it from
/// `tests/view/model.ts`: tasks with a text title, an integer rank, a set
/// of text tags and a link to a project, and three declared indexes;
/// projects with a text name and an index of the same name as one of the
/// tasks'.
pub fn lock() -> ModelLock {
    ModelLock::from_json(include_str!("../view/model.lock.json")).unwrap()
}

/// The view's model of [`lock`].
pub fn model() -> Model {
    Model::from_lock(&lock()).unwrap()
}

/// [`lock`] without the index `index` of `collection`: a lock of another
/// model.
pub fn lock_without(collection: &str, index: &str) -> ModelLock {
    let mut lock = lock();
    let collection = lock.collections.get_mut(collection).unwrap();
    collection.indexes.remove(index).unwrap();
    lock
}

/// A path for a database file no other test uses. Natively the directory
/// is the system's temporary one, and the file is removed when the value
/// drops. On wasm32 the path names a file of SQLite's in-memory VFS, the
/// default there (RFC 0006, "The browser"), which keeps it for the test
/// binary's run, across connections, as a file would be kept.
pub struct TempDb(pub PathBuf);

impl TempDb {
    pub fn new(name: &str) -> TempDb {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let db = TempDb(temp_path(&format!("ncdb-view-{name}-{n}.sqlite")));
        db.remove();
        db
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn remove(&self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }

    /// The in-memory VFS frees a file when the test binary ends.
    #[cfg(target_arch = "wasm32")]
    fn remove(&self) {}
}

#[cfg(not(target_arch = "wasm32"))]
fn temp_path(file: &str) -> PathBuf {
    std::env::temp_dir().join(format!("{}-{file}", std::process::id()))
}

#[cfg(target_arch = "wasm32")]
fn temp_path(file: &str) -> PathBuf {
    PathBuf::from(format!("/{file}"))
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
    fn model_id(&self) -> [u8; 16] {
        self.model.id
    }

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
        let mut state = self.fields.get(&key).cloned().unwrap_or(field.empty());
        match (&field, &mut state) {
            (Field::Basic(t), FieldState::Basic(s)) => apply(t, s, operation, origin)?,
            (Field::Link(t), FieldState::Basic(s)) => apply(t, s, operation, origin)?,
            (Field::Ref(t), FieldState::Basic(s)) => apply(t, s, operation, origin)?,
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

    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str) {
        self.fields
            .remove(&(object, collection.to_owned(), field.to_owned()));
    }

    fn clear_object(&mut self, object: ObjectId) {
        self.objects.remove(&object);
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
/// open: the view's and the test's. Natively a shared-cache database in
/// memory; on wasm32 a file of the in-memory VFS, which every connection
/// of the test binary shares by name.
pub struct MemDb(pub PathBuf);

impl MemDb {
    pub fn new() -> MemDb {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        MemDb(mem_path(n))
    }

    pub fn connect(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.0).unwrap()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn mem_path(n: u64) -> PathBuf {
    PathBuf::from(format!("file:ncdb-view-{n}?mode=memory&cache=shared"))
}

#[cfg(target_arch = "wasm32")]
fn mem_path(n: u64) -> PathBuf {
    PathBuf::from(format!("/ncdb-view-mem-{n}"))
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
