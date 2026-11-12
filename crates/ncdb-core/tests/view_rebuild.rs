//! A full rebuild of the view runs in the background (RFC 0006,
//! "Rebuilding the view", "How"; RFC 0009, "What `nc.open` does", step 4;
//! NOC-156, NOC-227). Opening returns at once. The app keeps reading the
//! old view, and new changes keep folding into it, while the rebuild
//! folds every frame of the log, a page at a time, into staging tables
//! beside the old ones (prefix `r_`). One transaction then swaps them in.
//! The progress counts frames. A rebuild cut off by a close starts again
//! at the next open: no marker is kept.
//!
//! The rebuild here is an app rolled back to an older lock. The old view
//! was folded by the stored lock's model, which keeps serving reads and
//! new changes until the swap.

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
use ncdb_core::model_lock::ModelLock;
use ncdb_core::receive::Checks;
use ncdb_core::view::{RebuildProgress, View};

use space::Space;
use view_support::*;

/// The space's creator, who writes the tasks.
const A: u8 = 1;
/// The member whose view this is: a writer, with the space key.
const B: u8 = 2;

const ANY: OperationId = OperationId {
    change: ChangeId([0; 32]),
    position: 0,
};

/// [`lock`] one version before, without the tasks' rank and the indexes
/// on it: an app rolled back to it opens a database written with
/// [`lock`].
fn older() -> ModelLock {
    let mut lock = lock();
    let task = lock.collections.get_mut(TASK).unwrap();
    task.fields.remove(RANK).unwrap();
    task.indexes
        .retain(|_, index| index.fields.iter().all(|f| f.field != RANK));
    lock.model.version -= 1;
    lock
}

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(B))
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

/// A space in which B is a writer, and `n` ranked tasks of A's.
fn space_with(n: u8) -> (Space, Vec<Change>) {
    let mut space = Space::new(A);
    let genesis = space.genesis.clone();
    let add = space.add(A, B, Role::Writer, &[&genesis]);
    let envelope = space.envelope(A, B, &[&add]);
    let mut changes = vec![genesis, add, envelope];
    for i in 1..=n {
        changes.push(space.sealed(
            A,
            vec![
                field_op::create(object(i), TASK),
                rank(object(i), u64::from(i)),
            ],
        ));
    }
    (space, changes)
}

/// Writes `changes` into B's view in `db`, opened with `lock`.
fn write(db: &TempDb, lock: &ModelLock, space: SpaceId, changes: &[Change]) {
    let mut view = View::open_with(&db.0, lock, checks_for).unwrap();
    let mut checks = checks_for(space);
    for change in changes {
        view.receive(change.bytes(), &mut checks).unwrap();
    }
}

fn ranked(view: &View, space: SpaceId, n: u8) -> bool {
    let row = view.get(space, TASK, object(n)).unwrap().unwrap();
    row.fields.contains_key(RANK)
}

fn parked_rank() -> State {
    State::Folded {
        parked: vec![ParkedOperation {
            position: 1,
            reason: OperationParkReason::UnknownField,
        }],
    }
}

/// Runs the rebuild to its end, `frames` at a time, and returns each
/// progress it reported.
fn finish(view: &mut View, frames: usize) -> Vec<RebuildProgress> {
    let mut seen = Vec::new();
    while let Some(progress) = view.rebuild_step(frames).unwrap() {
        seen.push(progress);
        assert!(seen.len() < 1000, "the rebuild ends");
    }
    seen
}

/// The names of the tables of the database whose name starts with `r_`.
fn staging_tables(db: &TempDb) -> Vec<String> {
    let conn = rusqlite::Connection::open(&db.0).unwrap();
    let mut statement = conn
        .prepare(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name LIKE 'r\\_%' ESCAPE '\\'",
        )
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn opening_returns_at_once_and_the_old_view_serves_reads_while_the_rebuild_runs() {
    let db = TempDb::new("rebuild-open");
    let (space, changes) = space_with(5);
    write(&db, &lock(), space.space, &changes);

    let view = View::open_with(&db.0, &older(), checks_for).unwrap();

    assert_eq!(
        view.rebuild_progress(),
        Some(RebuildProgress {
            folded: 0,
            total: 8
        })
    );
    // The old view, folded by the newer model, still answers.
    assert!(ranked(&view, space.space, 1));
    assert_eq!(
        view.state(space.space, changes[3].id()),
        Some(State::Folded { parked: Vec::new() })
    );
}

#[test]
fn the_rebuild_folds_a_page_at_a_time_and_swaps_the_new_view_in_at_its_end() {
    let db = TempDb::new("rebuild-pages");
    let (space, changes) = space_with(5);
    write(&db, &lock(), space.space, &changes);
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();

    let seen = finish(&mut view, 3);

    assert_eq!(
        seen,
        [
            RebuildProgress {
                folded: 3,
                total: 8
            },
            RebuildProgress {
                folded: 6,
                total: 8
            },
        ]
    );
    assert_eq!(view.rebuild_progress(), None);
    assert!(!view.model().collections[TASK].contains_key(RANK));
    assert_eq!(
        view.state(space.space, changes[3].id()),
        Some(parked_rank())
    );
    assert!(!ranked(&view, space.space, 1));
    assert!(view.get(space.space, TASK, object(1)).unwrap().is_some());
    drop(view);
    assert_eq!(staging_tables(&db), Vec::<String>::new());
}

#[test]
fn a_change_received_during_the_rebuild_shows_at_once_and_is_rebuilt_too() {
    let db = TempDb::new("rebuild-live");
    let (mut space, changes) = space_with(2);
    write(&db, &lock(), space.space, &changes);
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    view.rebuild_step(2).unwrap();
    let mut checks = checks_for(space.space);
    // The genesis, the add and the envelope: the control changes.
    for control in &changes[..3] {
        checks.add_control(control);
    }
    let late = space.sealed(
        A,
        vec![field_op::create(object(9), TASK), rank(object(9), 9)],
    );

    view.receive(late.bytes(), &mut checks).unwrap();

    assert!(
        ranked(&view, space.space, 9),
        "the old view folds it at once"
    );
    finish(&mut view, 2);
    assert_eq!(view.state(space.space, late.id()), Some(parked_rank()));
    assert!(view.get(space.space, TASK, object(9)).unwrap().is_some());
}

#[test]
fn a_rebuild_cut_off_by_a_close_starts_again_at_the_next_open() {
    let db = TempDb::new("rebuild-cut");
    let (space, changes) = space_with(5);
    write(&db, &lock(), space.space, &changes);
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    view.rebuild_step(4).unwrap();
    drop(view);
    assert!(!staging_tables(&db).is_empty());

    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();

    assert_eq!(
        view.rebuild_progress(),
        Some(RebuildProgress {
            folded: 0,
            total: 8
        })
    );
    finish(&mut view, 100);
    assert_eq!(
        view.state(space.space, changes[7].id()),
        Some(parked_rank())
    );
}

#[test]
fn the_rebuilt_view_equals_a_fold_under_the_new_model_from_the_start() {
    let rebuilt = TempDb::new("rebuild-same-rebuilt");
    let fresh = TempDb::new("rebuild-same-fresh");
    let (space, changes) = space_with(4);
    write(&rebuilt, &lock(), space.space, &changes);
    write(&fresh, &older(), space.space, &changes);

    let mut rebuilt = View::open_with(&rebuilt.0, &older(), checks_for).unwrap();
    finish(&mut rebuilt, 2);
    let fresh = View::open(&fresh.0, &older()).unwrap();

    for change in &changes {
        assert_eq!(
            rebuilt.state(space.space, change.id()),
            fresh.state(space.space, change.id())
        );
    }
    for n in 1..=4 {
        assert_eq!(
            format!("{:?}", rebuilt.get(space.space, TASK, object(n)).unwrap()),
            format!("{:?}", fresh.get(space.space, TASK, object(n)).unwrap())
        );
    }
}
