//! An additive change of the model migrates the view in place (RFC 0006,
//! "Rebuilding the view": "Add the column or table, and fold the parked
//! operations that are now known. No full rebuild."; RFC 0009, "What
//! `nc.open` does", step 4; NOC-156). A new declared index is a `CREATE
//! INDEX`. A new field or collection adds its column or tables, and only
//! the changes with operations parked as unknown are opened and folded
//! again. The result is the view a full refold gives. Any other change of
//! the model folds the whole log again, as before.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::State;
use ncdb_core::membership::Role;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::receive::{Checks, Stop};
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The space's creator, who writes the tasks.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

/// An id the encodings ignore: a change's operations take theirs from it.
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// [`lock`] one version before, without the tasks' rank and the indexes
/// on it.
fn without_rank() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

/// [`lock`] one version before, without the projects and the tasks' link
/// to them.
fn without_projects() -> ModelLock {
    let mut lock = lock();
    lock.collections.remove(PROJECT).unwrap();
    lock.collections
        .get_mut(TASK)
        .unwrap()
        .fields
        .remove(OWNER)
        .unwrap();
    lock.model.version -= 1;
    lock
}

/// [`lock`] one version before, without the declared index `inSpace`.
fn without_index() -> ModelLock {
    let mut lock = lock_without(TASK, "inSpace");
    lock.model.version -= 1;
    lock
}

/// Checks that count how many changes they open, as a full refold opens
/// every sealed change of the log and a migration only those it folds
/// again.
struct Counting {
    inner: SealChecks,
    opened: Rc<Cell<usize>>,
}

impl Checks for Counting {
    fn knows_suite(&self, suite: u64) -> bool {
        self.inner.knows_suite(suite)
    }
    fn verify(&self, change: &Change) -> Result<(), Stop> {
        self.inner.verify(change)
    }
    fn is_member(&self, change: &Change) -> bool {
        self.inner.is_member(change)
    }
    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        self.opened.set(self.opened.get() + 1);
        self.inner.open(change)
    }
    fn still_opens(&self, change: &Change) -> Result<(), Stop> {
        self.inner.still_opens(change)
    }
    fn open_folded(&self, change: &Change) -> Result<Payload, Stop> {
        self.opened.set(self.opened.get() + 1);
        self.inner.open_folded(change)
    }
    fn space_model_id(&self) -> Option<[u8; 16]> {
        self.inner.space_model_id()
    }
    fn add_control(&mut self, change: &Change) {
        self.inner.add_control(change)
    }
}

/// B's checks, as the member opens them: they hold no control change.
fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
}

/// A factory of [`Counting`] checks, and the count they share.
fn counting() -> (impl FnMut(SpaceId) -> Counting, Rc<Cell<usize>>) {
    let opened = Rc::new(Cell::new(0));
    let shared = opened.clone();
    let factory = move |space| Counting {
        inner: checks_for(space),
        opened: shared.clone(),
    };
    (factory, opened)
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

fn rank(object: ObjectId, value: u64) -> Operation {
    let write = Write {
        id: ANY,
        author: space::id(A),
        stamp: Hlc(0),
        value: Value::Unsigned(value),
        replaces: BTreeSet::new(),
    };
    let integer = BasicValue {
        kind: Kind::Integer,
    };
    field_op::encode(&integer, &write, object, TASK, RANK)
}

fn title(object: ObjectId, value: &str) -> Operation {
    let write = Write {
        id: ANY,
        author: space::id(A),
        stamp: Hlc(0),
        value: Value::Text(value.into()),
        replaces: BTreeSet::new(),
    };
    let text = BasicValue { kind: Kind::Text };
    field_op::encode(&text, &write, object, TASK, TITLE)
}

/// The changes of a space in which B is a writer: the control changes,
/// then `tasks`, each a change of A with these operations.
fn changes(tasks: Vec<Vec<Operation>>) -> (SpaceId, Vec<Change>) {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let mut all = vec![genesis, add, envelope];
    for operations in tasks {
        all.push(space.sealed(A, operations));
    }
    (space.space, all)
}

/// Writes `changes` into B's view in `db`, opened with `lock`, and closes
/// it again.
fn write(db: &TempDb, lock: &ModelLock, space: SpaceId, changes: &[Change]) {
    let mut view = View::open_with(&db.0, lock, checks_for).unwrap();
    let mut checks = checks_for(space);
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
}

/// Ten tasks with a title; the last one also ranked.
fn titled_and_one_ranked() -> Vec<Vec<Operation>> {
    (1..=10)
        .map(|n| {
            let mut operations = vec![field_op::create(object(n), TASK), title(object(n), "t")];
            if n == 10 {
                operations.push(rank(object(n), 3));
            }
            operations
        })
        .collect()
}

/// What `view` holds of `space`: each change's state, and each task's
/// fields.
fn snapshot(view: &View, space: SpaceId, changes: &[Change]) -> Vec<String> {
    let states = changes
        .iter()
        .map(|change| format!("{:?}", view.state(space, change.id())));
    let rows = (1..=10).map(|n| format!("{:?}", view.get(space, TASK, object(n)).unwrap()));
    states.chain(rows).collect()
}

#[test]
fn a_new_field_opens_only_the_changes_with_operations_parked_for_it() {
    let db = TempDb::new("migrate-field");
    let (space, changes) = changes(titled_and_one_ranked());
    write(&db, &without_rank(), space, &changes);
    let (factory, opened) = counting();

    let view = View::open_with(&db.0, &lock(), factory).unwrap();

    assert_eq!(opened.get(), 1, "only the ranked task is opened again");
    let ranked = changes.last().unwrap();
    assert_eq!(
        view.state(space, ranked.id()),
        Some(State::Folded { parked: Vec::new() })
    );
    let row = view.get(space, TASK, object(10)).unwrap().unwrap();
    assert!(row.fields.contains_key(RANK));
}

#[test]
fn a_migration_gives_the_view_a_full_fold_under_the_new_model_gives() {
    let migrated = TempDb::new("migrate-same-migrated");
    let fresh = TempDb::new("migrate-same-fresh");
    let (space, changes) = changes(titled_and_one_ranked());
    write(&migrated, &without_rank(), space, &changes);
    write(&fresh, &lock(), space, &changes);

    let migrated = View::open_with(&migrated.0, &lock(), checks_for).unwrap();
    let fresh = View::open(&fresh.0, &lock()).unwrap();

    assert_eq!(
        snapshot(&migrated, space, &changes),
        snapshot(&fresh, space, &changes)
    );
}

#[test]
fn a_new_field_with_nothing_parked_for_it_opens_without_checks() {
    let db = TempDb::new("migrate-nothing-parked");
    let tasks = (1..=3)
        .map(|n| vec![field_op::create(object(n), TASK), title(object(n), "t")])
        .collect();
    let (space, changes) = changes(tasks);
    write(&db, &without_rank(), space, &changes);

    let view = View::open(&db.0, &lock()).unwrap();

    assert!(view.model().collections[TASK].contains_key(RANK));
    assert!(view.get(space, TASK, object(1)).unwrap().is_some());
}

#[test]
fn a_new_collection_adds_its_tables_and_folds_what_was_parked_for_it() {
    let db = TempDb::new("migrate-collection");
    let project = object(0x50);
    let tasks = vec![
        vec![field_op::create(object(1), TASK), title(object(1), "t")],
        vec![field_op::create(project, PROJECT)],
    ];
    let (space, changes) = changes(tasks);
    write(&db, &without_projects(), space, &changes);
    let (factory, opened) = counting();

    let view = View::open_with(&db.0, &lock(), factory).unwrap();

    assert_eq!(opened.get(), 1, "only the project's change is opened again");
    assert!(view.get(space, PROJECT, project).unwrap().is_some());
    assert_eq!(
        view.state(space, changes.last().unwrap().id()),
        Some(State::Folded { parked: Vec::new() })
    );
}

#[test]
fn a_new_declared_index_is_created_without_folding_anything() {
    let db = TempDb::new("migrate-index");
    let tasks = vec![vec![field_op::create(object(1), TASK), rank(object(1), 3)]];
    let (space, changes) = changes(tasks);
    write(&db, &without_index(), space, &changes);

    let view = View::open(&db.0, &lock()).unwrap();
    drop(view);

    let conn = rusqlite::Connection::open(&db.0).unwrap();
    let indexes: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type = 'index' AND name LIKE '%inSpace%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(indexes, 1);
    let view = View::open(&db.0, &lock()).unwrap();
    assert!(view.get(space, TASK, object(1)).unwrap().is_some());
}
