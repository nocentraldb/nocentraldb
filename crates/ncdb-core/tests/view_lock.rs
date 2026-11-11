//! The check on the device (RFC 0005, "Two checks: at build and on the
//! device"; RFC 0009, "What `nc.open` does", steps 3 and 4): the view
//! keeps the lock its database was last opened with, runs the additive
//! check between it and the lock it is opened with now, refuses a lock of
//! another model or a change that is not additive and changes nothing,
//! and folds the log again for an additive change, so that what the old
//! model parked is folded.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{OperationParkReason, ParkedOperation, State};
use ncdb_core::membership::Role;
use ncdb_core::model_lock::{ModelId, ModelLock, Refusal};
use ncdb_core::view::{Error, View};

use space::Space;
use view_support::*;

/// The space's creator, who writes the tasks.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

const T: ObjectId = ObjectId([0x11; 16]);

/// An id the encodings ignore: a change's operations take theirs from it.
const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// [`lock`] as the model was one version before: without the tasks' rank,
/// and so without the indexes on it.
fn older() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

/// [`lock`] one version on, without the tasks' title: not additive.
fn title_removed() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(TITLE).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != TITLE));
    lock.model.version += 1;
    lock
}

/// [`lock`] with another `model.id` and nothing else changed.
fn other_model() -> ModelLock {
    let mut lock = lock();
    lock.model.id = ModelId([0x99; 16]);
    lock
}

/// B's checks, as the member opens them: they hold no control change.
fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
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

/// B's view in `db`, opened with `lock`, after the control changes that
/// make B a writer and a change of A that creates a task and ranks it;
/// closed again. The change's id.
fn written(db: &TempDb, lock: &ModelLock) -> (SpaceId, ChangeId) {
    let mut space = Space::new(A);
    let mut view = View::open_with(&db.0, lock, checks_for).unwrap();
    let mut checks = checks_for(space.space);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let task = space.sealed(A, vec![field_op::create(T, TASK), rank(T, 3)]);
    for change in [&genesis, &add, &envelope, &task] {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
    (space.space, task.id())
}

fn ranked(view: &View, space: SpaceId) -> bool {
    let row = view.get(space, TASK, T).unwrap().unwrap();
    row.object.exists() && row.fields.contains_key(RANK)
}

fn parked_rank() -> State {
    State::Folded {
        parked: vec![ParkedOperation {
            position: 1,
            reason: OperationParkReason::UnknownField,
        }],
    }
}

fn refusals(result: Result<View, Error>) -> Vec<Refusal> {
    match result {
        Err(Error::Refused(refusals)) => refusals,
        other => panic!("expected a refused lock, got {other:?}"),
    }
}

#[test]
fn a_lock_of_another_model_is_refused_and_names_both_ids() {
    let db = TempDb::new("lock-other-model");
    let (space, _) = written(&db, &lock());

    let refused = refusals(View::open_with(&db.0, &other_model(), checks_for));

    assert_eq!(
        refused,
        vec![Refusal::OtherModel {
            previous: lock().model.id,
            next: ModelId([0x99; 16]),
        }]
    );
    let lines = refused[0].lines().join(" ");
    assert!(lines.contains(&lock().model.id.to_string()));
    assert!(lines.contains(&ModelId([0x99; 16]).to_string()));
    // Without checks too: the view's model is the same, the lineage not.
    refusals(View::open(&db.0, &other_model()));
    let view = View::open(&db.0, &lock()).unwrap();
    assert!(ranked(&view, space));
}

#[test]
fn a_change_that_is_not_additive_is_refused_and_changes_nothing() {
    let db = TempDb::new("lock-refused");
    let (space, _) = written(&db, &lock());

    let refused = refusals(View::open_with(&db.0, &title_removed(), checks_for));

    assert!(refused.contains(&Refusal::FieldRemoved {
        collection: TASK.into(),
        field: TITLE.into(),
        rename: None,
    }));
    let view = View::open(&db.0, &lock()).unwrap();
    assert!(ranked(&view, space));
}

#[test]
fn an_additive_change_folds_what_the_old_model_parked() {
    let db = TempDb::new("lock-additive");
    let (space, task) = written(&db, &older());
    let view = View::open(&db.0, &older()).unwrap();
    assert_eq!(view.state(space, task), Some(parked_rank()));
    assert!(!ranked(&view, space));
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();

    assert_eq!(
        view.state(space, task),
        Some(State::Folded { parked: Vec::new() })
    );
    assert!(ranked(&view, space));
    drop(view);
    // The view now matches the lock, so it opens without checks.
    let view = View::open(&db.0, &lock()).unwrap();
    assert!(ranked(&view, space));
}

#[test]
fn an_older_lock_opens_the_database_as_an_old_client() {
    let db = TempDb::new("lock-rollback");
    let (space, task) = written(&db, &lock());

    let view = View::open_with(&db.0, &older(), checks_for).unwrap();

    assert_eq!(view.state(space, task), Some(parked_rank()));
    assert!(!ranked(&view, space));
    drop(view);
    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    assert!(ranked(&view, space));
}

/// The database keeps the newest lock it was opened with: after an older
/// lock opened it, a lock that is a successor of the older one but not of
/// the newest is refused.
#[test]
fn an_older_lock_does_not_let_a_lock_that_forks_from_it_in() {
    let db = TempDb::new("lock-fork");
    written(&db, &lock());
    drop(View::open_with(&db.0, &older(), checks_for).unwrap());
    let mut fork = older();
    fork.model.version = lock().model.version;
    let mut text = lock().collections[TASK].fields[TITLE].clone();
    text.since = fork.model.version;
    fork.collections
        .get_mut(TASK)
        .unwrap()
        .fields
        .insert(RANK.into(), text);

    let refused = refusals(View::open_with(&db.0, &fork, checks_for));

    assert!(
        refused
            .iter()
            .any(|r| matches!(r, Refusal::TypeChanged { field, .. } if field == RANK))
    );
}

#[test]
fn a_lock_that_changes_no_part_of_the_view_opens_without_checks() {
    let db = TempDb::new("lock-description");
    let (space, _) = written(&db, &lock());
    let mut described = lock();
    described
        .collections
        .get_mut(TASK)
        .unwrap()
        .fields
        .get_mut(TITLE)
        .unwrap()
        .description = Some("What the task is about.".into());

    let view = View::open(&db.0, &described).unwrap();

    assert!(ranked(&view, space));
}

/// Folding again needs the member's checks, which [`View::open`] does not
/// have: it refuses a change of the view's model that [`View::open_with`]
/// would fold.
#[test]
fn a_change_that_needs_folding_again_is_refused_without_checks() {
    let db = TempDb::new("lock-no-checks");
    written(&db, &older());

    assert!(matches!(View::open(&db.0, &lock()), Err(Error::Model)));
    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    assert!(view.model().collections[TASK].contains_key(RANK));
}

/// A refused lock leaves the view's tables and `meta` as they were.
#[test]
fn a_refused_lock_writes_nothing() {
    let db = TempDb::new("lock-nothing");
    written(&db, &lock());
    let before = view_bytes(&rusqlite::Connection::open(&db.0).unwrap());
    let meta = |db: &TempDb| -> Vec<(String, String)> {
        let conn = rusqlite::Connection::open(&db.0).unwrap();
        let mut statement = conn
            .prepare("SELECT key, value FROM meta ORDER BY key")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let meta_before = meta(&db);

    refusals(View::open_with(&db.0, &title_removed(), checks_for));
    refusals(View::open_with(&db.0, &other_model(), checks_for));

    assert_eq!(
        view_bytes(&rusqlite::Connection::open(&db.0).unwrap()),
        before
    );
    assert_eq!(meta(&db), meta_before);
}

/// Folding again keeps the log as it was: each change once, in its
/// order, with the relay's position.
#[test]
fn folding_again_keeps_the_log() {
    let db = TempDb::new("lock-log");
    let (space, task) = written(&db, &older());
    let mut view = View::open(&db.0, &older()).unwrap();
    view.confirm(space, task, 7).unwrap();
    let before: Vec<Change> = view
        .log(space)
        .unwrap()
        .after(ncdb_core::log::Position::START)
        .map(|(_, change)| change.clone())
        .collect();
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();

    let after: Vec<Change> = view
        .log(space)
        .unwrap()
        .after(ncdb_core::log::Position::START)
        .map(|(_, change)| change.clone())
        .collect();
    assert_eq!(after, before);
    assert_eq!(view.relay_position(space, task), Some(7));
}
