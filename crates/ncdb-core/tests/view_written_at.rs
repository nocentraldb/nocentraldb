//! When the member wrote each of its own changes (RFC 0002, "The log on
//! disk"; RFC 0006, "What the SDK reports"; the owner's NOC-231 point 5):
//! the `log` table's `written_at`, milliseconds since the Unix epoch, set
//! when the member appends a change it wrote, and `NULL` for every change
//! it received. The oldest of them that has no relay position is the
//! durability report's `oldestUnconfirmedAt`. The column is additive: a
//! database from before gains it, its rows `NULL`, and the view format
//! version stays 2.

mod support;

#[path = "support/space.rs"]
mod space;
#[path = "support/view.rs"]
mod view_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::ObjectId;
use ncdb_core::view::View;

use space::Space;
use view_support::*;

/// The member whose view this is: the space's creator.
const ME: u8 = 1;

/// 2026-11-12T08:00:00Z, in milliseconds since the Unix epoch.
const MORNING: u64 = 1_794_384_000_000;

/// The view's checks for its member.
fn checks(space: &Space) -> SealChecks {
    SealChecks::new(space.space, space::key(ME))
}

/// A task `n` of the member, sealed in `space`.
fn task(space: &mut Space, n: u8) -> Change {
    space.sealed(ME, vec![field_op::create(ObjectId([n; 16]), TASK)])
}

/// A view in memory holding the genesis, received as it would be from the
/// relay, so it keeps no time.
fn view_with_genesis(space: &Space) -> View {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.receive(space.genesis.bytes(), &mut checks(space))
        .unwrap();
    view
}

#[test]
fn an_own_change_keeps_the_time_it_was_written() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let task = task(&mut space, 1);

    view.receive_own(task.bytes(), &mut checks(&space), MORNING)
        .unwrap();

    assert_eq!(view.written_at(space.space, task.id()), Some(MORNING));
    assert_eq!(view.written_at(space.space, space.genesis.id()), None);
}

#[test]
fn a_view_that_received_every_change_holds_no_own_unconfirmed_time() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let task = task(&mut space, 1);
    view.receive(task.bytes(), &mut checks(&space)).unwrap();

    assert_eq!(view.oldest_unconfirmed_own().unwrap(), None);
}

#[test]
fn the_oldest_is_the_earliest_own_change_without_a_relay_position() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let first = task(&mut space, 1);
    let second = task(&mut space, 2);
    view.receive_own(first.bytes(), &mut checks(&space), MORNING)
        .unwrap();
    view.receive_own(second.bytes(), &mut checks(&space), MORNING + 5)
        .unwrap();
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));

    view.confirm(space.space, first.id(), 1).unwrap();
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING + 5));

    view.confirm(space.space, second.id(), 2).unwrap();
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), None);
}

#[test]
fn a_refused_own_change_stays_unconfirmed() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let task = task(&mut space, 1);
    view.receive_own(task.bytes(), &mut checks(&space), MORNING)
        .unwrap();

    view.refuse(space.space, task.id(), "not_allowed").unwrap();

    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));
}

#[test]
fn a_new_relay_epoch_makes_confirmed_own_changes_unconfirmed_again() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let task = task(&mut space, 1);
    view.receive_own(task.bytes(), &mut checks(&space), MORNING)
        .unwrap();
    view.confirm(space.space, task.id(), 1).unwrap();
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), None);

    view.forget_relay_positions().unwrap();

    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));
}

#[test]
fn receive_all_own_keeps_one_time_for_every_new_frame() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let first = task(&mut space, 1);
    let second = task(&mut space, 2);

    view.receive_all_own(
        &[first.bytes(), second.bytes()],
        &mut checks(&space),
        MORNING,
    )
    .unwrap();

    assert_eq!(view.written_at(space.space, first.id()), Some(MORNING));
    assert_eq!(view.written_at(space.space, second.id()), Some(MORNING));
}

#[test]
fn a_duplicate_keeps_the_first_time() {
    let mut space = Space::new(ME);
    let mut view = view_with_genesis(&space);
    let task = task(&mut space, 1);
    view.receive_own(task.bytes(), &mut checks(&space), MORNING)
        .unwrap();

    view.receive_own(task.bytes(), &mut checks(&space), MORNING + 9)
        .unwrap();
    view.receive(task.bytes(), &mut checks(&space)).unwrap();

    assert_eq!(view.written_at(space.space, task.id()), Some(MORNING));
}

/// The column's layout, part of the log's on-disk format (NOC-231): an
/// `INTEGER` that may be `NULL`, after the columns that were there.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_log_holds_written_at_as_a_nullable_integer() {
    let db = TempDb::new("written-at-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = rusqlite::Connection::open(&db.0).unwrap();
    let column: (String, bool) = conn
        .query_row(
            "SELECT type, \"notnull\" FROM pragma_table_info('log') WHERE name = 'written_at'",
            [],
            |row| Ok((row.get(0)?, row.get::<_, i64>(1)? == 1)),
        )
        .unwrap();
    assert_eq!(column, ("INTEGER".to_owned(), false));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn written_at_survives_a_reopen() {
    let db = TempDb::new("written-at-reopen");
    let mut space = Space::new(ME);
    let task = task(&mut space, 1);
    {
        let mut view = View::open(&db.0, &lock()).unwrap();
        view.receive(space.genesis.bytes(), &mut checks(&space))
            .unwrap();
        view.receive_own(task.bytes(), &mut checks(&space), MORNING)
            .unwrap();
    }

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.written_at(space.space, task.id()), Some(MORNING));
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));
}

/// A database written before the column existed opens, gains it with
/// `NULL` in every row it held, and keeps its format version: the column is
/// additive, and a core that does not know it ignores it.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_database_from_before_the_column_gains_it_and_keeps_its_format_version() {
    let db = TempDb::new("written-at-before");
    let mut space = Space::new(ME);
    let old = task(&mut space, 1);
    {
        let mut view = View::open(&db.0, &lock()).unwrap();
        view.receive(space.genesis.bytes(), &mut checks(&space))
            .unwrap();
        view.receive(old.bytes(), &mut checks(&space)).unwrap();
    }
    let conn = rusqlite::Connection::open(&db.0).unwrap();
    conn.execute_batch("ALTER TABLE log DROP COLUMN written_at")
        .unwrap();
    drop(conn);

    let mut view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.written_at(space.space, old.id()), None);
    let new = task(&mut space, 2);
    view.receive_own(new.bytes(), &mut checks(&space), MORNING)
        .unwrap();
    assert_eq!(view.oldest_unconfirmed_own().unwrap(), Some(MORNING));
    drop(view);

    let conn = rusqlite::Connection::open(&db.0).unwrap();
    let format: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(format, "2");
    assert_eq!(View::FORMAT, 2);
}
