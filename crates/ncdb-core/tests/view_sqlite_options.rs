//! The view relies on no SQLite option that only one build has (NOC-185,
//! decided as A by the owner): the native and the browser builds share the
//! SQLite release, not the compile options. Natively foreign keys are on by
//! default and FTS and R-Tree are built in; in the browser the page size is
//! 8192 and neither FTS nor R-Tree exists. So the view's schema holds no
//! virtual table and no foreign key, and what it reads does not depend on
//! the page size.
//!
//! It runs on both targets: natively, and on wasm32 in the gate's test
//! runner, on the SQLite build the browser uses (sqlite-wasm-rs). The plan
//! check, which `View::open` runs on both, stays as it is. The fixtures
//! show that the check fires: a foreign key on both targets, FTS and R-Tree
//! natively, since the browser build cannot even create them.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId};
use ncdb_core::view::View;
use rusqlite::Connection;

use view_support::*;

/// What in the schema of the database `conn` relies on an option only one
/// SQLite build has: each virtual table, such as FTS or R-Tree, and each
/// table with a foreign key. Empty when the schema runs on both builds.
fn unportable(conn: &Connection) -> Vec<String> {
    let _ = conn;
    Vec::new()
}

/// A task with a title and a rank.
fn task(n: u8) -> Vec<Operation> {
    let object = ObjectId([n; 16]);
    let set = |field: &str, kind, value| {
        let write = Write {
            id: OperationId {
                change: ChangeId([0; 32]),
                position: 0,
            },
            author: member::member(member::FOUNDER),
            stamp: Hlc(0),
            value,
            replaces: BTreeSet::new(),
        };
        field_op::encode(&BasicValue { kind }, &write, object, TASK, field)
    };
    vec![
        field_op::create(object, TASK),
        set(TITLE, Kind::Text, Value::Text(format!("task {n}"))),
        set(RANK, Kind::Integer, Value::Unsigned(u64::from(n) * 7 % 31)),
    ]
}

/// The view of the database at `db`, with the key envelope and forty
/// tasks in four changes folded.
fn folded(db: &MemDb) -> View {
    let mut view = View::open(&db.0, &lock()).unwrap();
    let mut control = member::Control::default();
    view.receive(&member::envelope(member::FOUNDER, 1, 0), &mut control)
        .unwrap();
    for change in 0..4u8 {
        let operations = (0..10).flat_map(|n| task(change * 10 + n)).collect();
        let frame = member::sealed(member::FOUNDER, 2 + u64::from(change), 0, operations);
        view.receive(&frame, &mut control).unwrap();
    }
    view
}

#[test]
fn a_foreign_key_relies_on_an_option_one_build_lacks() {
    let db = MemDb::new();
    let conn = db.connect();
    conn.execute_batch(
        "CREATE TABLE parent (id INTEGER PRIMARY KEY);
         CREATE TABLE child (id INTEGER PRIMARY KEY, parent INTEGER REFERENCES parent(id));",
    )
    .unwrap();

    assert_eq!(unportable(&conn), ["child: a foreign key"]);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn fts_and_r_tree_rely_on_options_one_build_lacks() {
    let db = MemDb::new();
    let conn = db.connect();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE words USING fts5(text);
         CREATE VIRTUAL TABLE boxes USING rtree(id, min_x, max_x);",
    )
    .unwrap();

    assert_eq!(
        unportable(&conn),
        ["boxes: a virtual table", "words: a virtual table"]
    );
}

#[test]
fn the_views_schema_relies_on_no_option_one_build_lacks() {
    let db = MemDb::new();
    let _view = folded(&db);

    assert_eq!(unportable(&db.connect()), Vec::<String>::new());
}

#[test]
fn what_the_view_holds_does_not_depend_on_the_page_size() {
    let bytes: Vec<Vec<u8>> = [1024, 4096, 8192, 65536]
        .into_iter()
        .map(|page_size| {
            let db = MemDb::new();
            let conn = db.connect();
            // The page size holds from the first page written on.
            conn.execute_batch(&format!(
                "PRAGMA page_size = {page_size}; CREATE TABLE first (x); DROP TABLE first;"
            ))
            .unwrap();
            let size: u32 = conn
                .query_row("PRAGMA page_size", [], |row| row.get(0))
                .unwrap();
            assert_eq!(size, page_size, "the database's page size");
            let _view = folded(&db);
            view_bytes(&conn)
        })
        .collect();

    assert!(!bytes[0].is_empty());
    assert!(
        bytes.iter().all(|b| *b == bytes[0]),
        "a page size changed the view"
    );
}
