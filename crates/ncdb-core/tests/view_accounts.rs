//! The accounts a person confirmed (NOC-168; RFC 0009, "Invitations and
//! members"): the database keeps each account whose fingerprint a person
//! compared, by its public key and when it was confirmed, in the table
//! `confirmed_accounts` of the member's SQLite file. Unlike the view's
//! tables, it is not computed from the log, so nothing rebuilds it.
//!
//! Native only: the browser's view (OPFS) is a later ticket.
#![cfg(not(target_arch = "wasm32"))]

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::checks::SealChecks;
use ncdb_core::ids::SpaceId;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;
use rusqlite::Connection;

use view_support::*;

const ACCOUNT: [u8; 32] = [0xA1; 32];
const OTHER: [u8; 32] = [0xB2; 32];

/// 2026-11-10T12:00:00Z, in milliseconds since the Unix epoch.
const NOON: u64 = 1_794_312_000_000;

#[test]
fn an_account_nobody_confirmed_is_not_confirmed() {
    let view = View::open_in_memory(&lock()).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), None);
}

#[test]
fn a_confirmed_account_is_kept_with_the_time_it_was_confirmed() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), Some(NOON));
    assert_eq!(view.account_confirmed_at(&OTHER).unwrap(), None);
}

#[test]
fn confirming_an_account_again_keeps_the_first_time() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    view.confirm_account(&ACCOUNT, NOON + 1).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), Some(NOON));
}

#[test]
fn a_confirmed_account_survives_a_reopen() {
    let db = TempDb::new("accounts-reopen");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), Some(NOON));
}

/// The table's layout, which is part of the on-disk format the owner
/// approved (NOC-154 point 3 A): the account's public key and when it was
/// confirmed, nothing else.
#[test]
fn the_table_holds_the_account_key_and_when_it_was_confirmed_and_nothing_else() {
    let db = TempDb::new("accounts-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    let columns: Vec<(String, String, bool, bool)> = conn
        .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info('confirmed_accounts')")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, i64>(3)? == 1,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        columns,
        [
            ("account_key".to_owned(), "BLOB".to_owned(), true, true),
            ("confirmed_at".to_owned(), "INTEGER".to_owned(), true, false),
        ]
    );
    let strict: i64 = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE name = 'confirmed_accounts'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strict, 1);
}

#[test]
fn a_confirmed_account_is_stored_as_its_32_byte_key_and_milliseconds() {
    let db = TempDb::new("accounts-row");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    drop(view);
    let conn = Connection::open(&db.0).unwrap();
    let row: (Vec<u8>, i64) = conn
        .query_row(
            "SELECT account_key, confirmed_at FROM confirmed_accounts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, (ACCOUNT.to_vec(), NOON as i64));
}

/// A database written before the table existed (format version 2 without
/// it) opens, gains the table, and keeps its format version: the table is
/// additive, and a core that does not know it ignores it.
#[test]
fn a_database_from_before_the_table_gains_it_and_keeps_its_format_version() {
    let db = TempDb::new("accounts-before");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    conn.execute_batch("DROP TABLE confirmed_accounts").unwrap();
    drop(conn);

    let mut view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), None);
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), Some(NOON));
    drop(view);

    let conn = Connection::open(&db.0).unwrap();
    let format: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(format, "2");
    assert_eq!(View::FORMAT, 2);
}

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

fn checks_for(space: SpaceId) -> SealChecks {
    SealChecks::new(space, space::key(2))
}

/// Folding the log again for a changed model drops and rebuilds the view's
/// tables; the confirmations are no part of the view and stay.
#[test]
fn folding_the_log_again_keeps_the_confirmed_accounts() {
    let db = TempDb::new("accounts-refold");
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    view.confirm_account(&ACCOUNT, NOON).unwrap();
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    assert!(view.model().collections[TASK].contains_key(RANK));
    assert_eq!(view.account_confirmed_at(&ACCOUNT).unwrap(), Some(NOON));
}
