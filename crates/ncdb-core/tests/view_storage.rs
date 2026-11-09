//! The local view on SQLite, natively (NOC-69; RFC 0006, "Where the view
//! lives" and "Desktop and mobile"): the tables per collection, the
//! member's log stored with the view, one transaction per received change,
//! and the version of the view in `meta`.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use ncdb_core::change::{Change, Payload};
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::{Appended, RejectReason, State};
use ncdb_core::receive::{Checks, Stop};
use ncdb_core::view::{Error, View};
use rusqlite::Connection;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);

fn create(sequence: u64, generation: u64) -> Vec<u8> {
    sealed(
        FOUNDER,
        sequence,
        generation,
        vec![field_op::create(T, TASK)],
    )
}

fn id(frame: &[u8]) -> ncdb_core::ids::ChangeId {
    Change::decode(frame).unwrap().id()
}

#[test]
fn opening_a_view_creates_a_table_per_collection_the_log_and_meta() {
    let db = TempDb::new("tables");
    let _view = View::open(&db.0, model()).unwrap();
    let conn = Connection::open(&db.0).unwrap();
    let schema = schema(&conn);
    for table in [
        "log",
        "meta",
        "objects",
        "c_task",
        "fs_task",
        "c_project",
        "fs_project",
    ] {
        assert!(
            schema.contains(&("table".to_owned(), table.to_owned())),
            "{table} in {schema:?}"
        );
    }
    for index in ["ix_task_by_rank", "ix_task_by_title"] {
        assert!(schema.contains(&("index".to_owned(), index.to_owned())));
    }
    // One column per field with a single sortable value; a set has none
    // (RFC 0006: its read value is served from fs_task).
    let columns: BTreeSet<String> = conn
        .prepare("SELECT name FROM pragma_table_info('c_task')")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let expected = ["space_id", "object_id", "f_owner", "f_rank", "f_title"];
    assert_eq!(columns, expected.iter().map(|c| c.to_string()).collect());

    let format: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(format, View::FORMAT.to_string());
}

#[test]
fn the_database_runs_in_wal_mode() {
    let db = TempDb::new("wal");
    let _view = View::open(&db.0, model()).unwrap();
    let conn = Connection::open(&db.0).unwrap();
    let mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
}

#[test]
fn the_sqlite_in_the_binary_is_the_bundled_release_rfc_0006_names() {
    assert_eq!(rusqlite::version(), "3.54.1");
}

/// A row of the log table: space id, change id, state, reason, frame.
type LogRow = (Vec<u8>, Vec<u8>, String, Option<String>, Vec<u8>);

#[test]
fn a_received_change_is_in_the_log_with_its_frame_and_state() {
    let db = TempDb::new("log");
    let mut view = View::open(&db.0, model()).unwrap();
    let mut control = Control::default();
    let key = envelope(FOUNDER, 1, 0);
    let unknown = ncdb_core::change::Operation::new(T, "nope".into(), None, 0, None);
    let folded = sealed(FOUNDER, 2, 0, vec![field_op::create(T, TASK), unknown]);
    let no_key = create(3, 1);
    for frame in [&key, &folded, &no_key] {
        view.receive(frame, &mut control).unwrap();
    }

    let conn = Connection::open(&db.0).unwrap();
    let rows: Vec<LogRow> = conn
        .prepare(
            "SELECT space_id, change_id, state, reason, frame FROM log ORDER BY local_position",
        )
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let row = |frame: &[u8], state: &str, reason: Option<&str>| {
        (
            SPACE.0.to_vec(),
            id(frame).0.to_vec(),
            state.to_owned(),
            reason.map(str::to_owned),
            frame.to_vec(),
        )
    };
    assert_eq!(
        rows,
        vec![
            row(&key, "folded", None),
            row(&folded, "folded", None),
            row(&no_key, "rejected", Some("no_key")),
        ]
    );
    // The operation the model does not know is parked on its own (D10).
    let parked: Vec<(Vec<u8>, i64, String)> = conn
        .prepare("SELECT change_id, position, reason FROM parked")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        parked,
        vec![(id(&folded).0.to_vec(), 1, "unknown_collection".to_owned())]
    );
}

#[test]
fn reopening_the_view_keeps_the_log_and_the_states_of_its_changes() {
    let db = TempDb::new("reopen");
    let mut control = Control::default();
    let early = create(1, 0);
    {
        let mut view = View::open(&db.0, model()).unwrap();
        view.receive(&early, &mut control).unwrap();
        assert_eq!(
            view.state(SPACE, id(&early)),
            Some(State::Rejected(RejectReason::NoKey))
        );
    }
    let mut view = View::open(&db.0, model()).unwrap();
    assert_eq!(
        view.state(SPACE, id(&early)),
        Some(State::Rejected(RejectReason::NoKey))
    );
    assert!(matches!(
        view.receive(&early, &mut control).unwrap(),
        Appended::Duplicate(_)
    ));
    // The envelope that arrives after the reopen lets the rejected change
    // pass (D11), so its state was reloaded with it.
    view.receive(&envelope(FOUNDER, 2, 0), &mut control)
        .unwrap();
    assert_eq!(
        view.state(SPACE, id(&early)),
        Some(State::Folded { parked: Vec::new() })
    );
    drop(view);
    let view = View::open(&db.0, model()).unwrap();
    assert_eq!(
        view.state(SPACE, id(&early)),
        Some(State::Folded { parked: Vec::new() })
    );
}

#[test]
fn a_view_of_another_format_version_is_refused() {
    let db = TempDb::new("format");
    drop(View::open(&db.0, model()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    conn.execute("UPDATE meta SET value = '999' WHERE key = 'format'", [])
        .unwrap();
    drop(conn);
    assert!(matches!(
        View::open(&db.0, model()),
        Err(Error::Format { found: 999 })
    ));
}

#[test]
fn a_view_folded_by_another_model_is_refused() {
    let db = TempDb::new("model");
    drop(View::open(&db.0, model()).unwrap());
    let mut other = model();
    other.indexes.pop();
    assert!(matches!(View::open(&db.0, other), Err(Error::Model)));
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
    let a = create(1, 0);
    let b = sealed(
        FOUNDER,
        2,
        0,
        vec![field_op::create(ObjectId([2; 16]), TASK)],
    );
    let key = envelope(FOUNDER, 3, 0);
    let mut view = View::open(&db.0, model()).unwrap();
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
