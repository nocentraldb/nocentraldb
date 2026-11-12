//! When this device first held each provisional shred, in the table
//! `shreds_held` of the member's SQLite file (RFC 0008, "A provisional
//! erasure in a space with one account"; RFC 0009, "Sync and status":
//! `provisionalErasures`). Control changes carry no clock (RFC 0008, "The
//! control bodies"), so a shred's age, and the 7 days after which a device
//! of the account finalizes it, count from when this device first held it
//! (the owner's NOC-218 A). Like `confirmed_accounts`, the table is not
//! computed from the log, so nothing rebuilds it. Taken from the closed
//! #209 (NOC-198), as pm asked for NOC-234.

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::ChangeId;
use ncdb_core::view::View;
use rusqlite::Connection;

use space::Space;
use view_support::*;

/// The space's creator.
const A: u8 = 1;

/// 2026-11-10T12:00:00Z, in milliseconds since the Unix epoch.
const NOON: u64 = 1_794_312_000_000;

#[test]
fn a_shred_this_device_never_held_has_no_time() {
    let view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);
    assert_eq!(
        view.shred_held_at(space.space, ChangeId([7; 32])).unwrap(),
        None
    );
}

#[test]
fn a_shred_keeps_the_time_this_device_first_held_it() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    let space = Space::new(A);
    let other = Space::with_nonce(A, [9; 16]);
    let shred = ChangeId([7; 32]);
    view.note_shred_held(space.space, shred, NOON).unwrap();
    view.note_shred_held(space.space, shred, NOON + 1).unwrap();
    assert_eq!(view.shred_held_at(space.space, shred).unwrap(), Some(NOON));
    assert_eq!(view.shred_held_at(other.space, shred).unwrap(), None);
}

#[test]
fn the_time_a_shred_was_first_held_survives_a_reopen() {
    let db = TempDb::new("shreds-held-reopen");
    let space = Space::new(A);
    let shred = ChangeId([7; 32]);
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.note_shred_held(space.space, shred, NOON).unwrap();
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.shred_held_at(space.space, shred).unwrap(), Some(NOON));
}

/// The table's layout, an addition to the on-disk format for the owner's
/// tick (NOC-218): the space, the shred and when this device first held it,
/// nothing else.
#[test]
fn the_table_holds_the_space_the_shred_and_when_it_was_first_held() {
    let db = TempDb::new("shreds-held-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    let columns: Vec<(String, String, bool, i64)> = conn
        .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info('shreds_held')")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        columns,
        [
            ("space_id".to_owned(), "BLOB".to_owned(), true, 1),
            ("change_id".to_owned(), "BLOB".to_owned(), true, 2),
            ("held_at".to_owned(), "INTEGER".to_owned(), true, 0),
        ]
    );
    let strict: i64 = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE name = 'shreds_held'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strict, 1);
}
