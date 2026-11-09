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
