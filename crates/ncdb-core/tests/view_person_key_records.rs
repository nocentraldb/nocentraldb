//! The person-key records this device keeps, in the table
//! `person_key_records` of the member's SQLite file (RFC 0003, "The person
//! key"; NOC-547): a reopened database holds its person keys again from
//! its own copy, with no relay. Like `shreds_held`, the table is not
//! computed from the log, so nothing rebuilds it.

#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::SpaceId;
use ncdb_core::view::{PersonKeyRecord, View};
use rusqlite::Connection;

use view_support::*;

fn record(person_key: u8, generation: u64) -> PersonKeyRecord {
    PersonKeyRecord {
        person_key: [person_key; 16],
        generation,
        writer: [7; 32],
        wrapped: vec![1, 2, 3],
        signature: [9; 64],
    }
}

const SPACE: SpaceId = SpaceId([1; 16]);
const OTHER: SpaceId = SpaceId([2; 16]);

#[test]
fn a_kept_record_is_read_back_once_by_its_space() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.keep_person_key_record(SPACE, &record(5, 1), false).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 2), false).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 1), false).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 1), false).unwrap();
    view.keep_person_key_record(OTHER, &record(4, 1), false).unwrap();

    assert_eq!(
        view.person_key_records(SPACE).unwrap(),
        [record(4, 1), record(4, 2), record(5, 1)]
    );
    assert_eq!(view.person_key_records(OTHER).unwrap(), [record(4, 1)]);
}

#[test]
fn a_forgotten_person_key_keeps_no_record_in_its_space() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 1), false).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 2), false).unwrap();
    view.keep_person_key_record(SPACE, &record(5, 1), false).unwrap();
    view.keep_person_key_record(OTHER, &record(4, 1), false).unwrap();

    view.forget_person_key_records(SPACE, [4; 16]).unwrap();

    assert_eq!(view.person_key_records(SPACE).unwrap(), [record(5, 1)]);
    assert_eq!(view.person_key_records(OTHER).unwrap(), [record(4, 1)]);
}

// A record this member wrote waits to be put to a relay until the relay
// took it, across a reopen; a copy the relay served never waits.
#[test]
fn a_record_waits_to_be_put_until_the_relay_took_it() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 1), true).unwrap();
    view.keep_person_key_record(SPACE, &record(5, 1), true).unwrap();
    view.keep_person_key_record(SPACE, &record(6, 1), false).unwrap();
    view.keep_person_key_record(OTHER, &record(4, 1), true).unwrap();

    assert_eq!(
        view.waiting_person_key_records().unwrap(),
        [
            (SPACE, record(4, 1)),
            (SPACE, record(5, 1)),
            (OTHER, record(4, 1)),
        ]
    );
    view.person_key_records_put(SPACE, &[record(4, 1)]).unwrap();
    // The relay served it: it holds it.
    view.keep_person_key_record(SPACE, &record(5, 1), false).unwrap();
    // A record the relay served is not put again by a later keep.
    view.keep_person_key_record(SPACE, &record(6, 1), true).unwrap();

    assert_eq!(
        view.waiting_person_key_records().unwrap(),
        [(OTHER, record(4, 1))]
    );
    assert_eq!(view.person_key_records(SPACE).unwrap().len(), 3);
}

#[test]
fn the_records_survive_a_reopen() {
    let db = TempDb::new("person-key-records-reopen");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.keep_person_key_record(SPACE, &record(4, 1), true).unwrap();
    drop(view);

    let view = View::open(&db.0, &lock()).unwrap();

    assert_eq!(view.person_key_records(SPACE).unwrap(), [record(4, 1)]);
    assert_eq!(
        view.waiting_person_key_records().unwrap(),
        [(SPACE, record(4, 1))]
    );
}

/// The table's layout, an addition to the on-disk format: the space, the
/// record's person-key id, generation and writer, and the wrapped key and
/// signature, as the relay stores them (RFC 0007, `record`), and whether
/// it waits to be put to a relay.
#[test]
fn the_table_holds_the_record_as_the_relay_stores_it() {
    let db = TempDb::new("person-key-records-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    let columns: Vec<(String, String, bool, i64)> = conn
        .prepare(
            "SELECT name, type, \"notnull\", pk FROM pragma_table_info('person_key_records')",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let column = |name: &str, kind: &str, pk: i64| (name.to_owned(), kind.to_owned(), true, pk);
    assert_eq!(
        columns,
        [
            column("space_id", "BLOB", 1),
            column("person_key_id", "BLOB", 2),
            column("generation", "INTEGER", 3),
            column("writer", "BLOB", 4),
            column("wrapped", "BLOB", 0),
            column("signature", "BLOB", 0),
            column("waiting", "INTEGER", 0),
        ]
    );
    let strict: i64 = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE name = 'person_key_records'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strict, 1);
}
