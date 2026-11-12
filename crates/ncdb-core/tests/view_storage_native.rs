//! The local view on SQLite, natively only (NOC-69; RFC 0006, "Desktop and
//! mobile"): what holds on desktop and mobile and not in the browser.
//!
//! - WAL mode with `synchronous = FULL` is what RFC 0006 ("Desktop and mobile")
//!   takes for a file on a device. The browser runs one connection on
//!   `opfs-sahpool`, which has no shared-memory WAL (RFC 0006, "SQLite on each
//!   target"); the browser test (`scripts/browser-view/`) reports the journal
//!   mode it gets there.
//! - The crash in the middle of a fold is a caught panic, and a panic on
//!   wasm32 aborts the test binary.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, as in every test file (NOC-10), although
// this file is native only.
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ncdb_core::change::{Change, Payload};
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::receive::{Checks, Stop};
use ncdb_core::view::View;
use rusqlite::Connection;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);

#[test]
fn the_database_runs_in_wal_mode() {
    let db = TempDb::new("wal");
    let _view = View::open(&db.0, &lock()).unwrap();
    let conn = Connection::open(&db.0).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

/// The test double's checks, which panic on the `n`th decryption, as a
/// crash in the middle of a fold.
struct CrashOnOpen {
    control: Control,
    opens: Cell<u32>,
    crash_at: u32,
}

impl Checks for CrashOnOpen {
    fn knows_suite(&self, suite: u64) -> bool {
        self.control.knows_suite(suite)
    }

    fn verify(&self, change: &Change) -> Result<(), Stop> {
        self.control.verify(change)
    }

    fn is_member(&self, change: &Change) -> bool {
        self.control.is_member(change)
    }

    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        self.opens.set(self.opens.get() + 1);
        assert_ne!(self.opens.get(), self.crash_at, "crash");
        self.control.open(change)
    }

    fn add_control(&mut self, change: &Change) {
        self.control.add_control(change);
    }
}

#[test]
fn a_change_and_everything_its_fold_wrote_are_committed_together_or_not_at_all() {
    let db = TempDb::new("transaction");
    let a = sealed(FOUNDER, 1, 0, vec![field_op::create(T, TASK)]);
    let b = sealed(
        FOUNDER,
        2,
        0,
        vec![field_op::create(ObjectId([2; 16]), TASK)],
    );
    let key = envelope(FOUNDER, 3, 0);
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = CrashOnOpen {
        control: Control::default(),
        opens: Cell::new(0),
        crash_at: 4,
    };
    view.receive(&a, &mut checks).unwrap();
    view.receive(&b, &mut checks).unwrap();
    // The envelope lets both rejected changes pass: the fold writes `a`
    // into the view, then crashes on `b`.
    let crash = catch_unwind(AssertUnwindSafe(|| view.receive(&key, &mut checks)));
    assert!(crash.is_err());
    drop(view);

    let conn = Connection::open(&db.0).unwrap();
    let states: Vec<String> = conn
        .prepare("SELECT state FROM log ORDER BY local_position")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(states, ["rejected", "rejected"]);
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM c_task", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
    let objects: i64 = conn
        .query_row("SELECT count(*) FROM objects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(objects, 0);
}

/// A crash in the middle of a batch (NOC-204) leaves none of it: the
/// changes before the crash share its one transaction.
#[test]
fn a_crash_in_the_middle_of_a_batch_leaves_none_of_it() {
    let db = TempDb::new("batch-crash");
    let key = envelope(FOUNDER, 1, 0);
    let a = sealed(FOUNDER, 2, 0, vec![field_op::create(T, TASK)]);
    let b = sealed(
        FOUNDER,
        3,
        0,
        vec![field_op::create(ObjectId([2; 16]), TASK)],
    );
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut checks = CrashOnOpen {
        control: Control::default(),
        opens: Cell::new(0),
        crash_at: 2,
    };
    let crash = catch_unwind(AssertUnwindSafe(|| {
        view.receive_all(&[&key, &a, &b], &mut checks)
    }));
    assert!(crash.is_err());
    drop(view);

    let conn = Connection::open(&db.0).unwrap();
    let rows: i64 = conn
        .query_row("SELECT count(*) FROM log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
    let tasks: i64 = conn
        .query_row("SELECT count(*) FROM c_task", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tasks, 0);
}
