//! Which object each copy in an ad-hoc space came from, on every target
//! (NOC-318; RFC 0003, "Sub-agents get an ad-hoc space"; RFC 0009,
//! "Sub-agents and ad-hoc spaces"): the parent's database keeps the mapping
//! from each copy to its source in the table `ad_hoc_copies` of the
//! member's SQLite file, on the parent only. Unlike the view's tables, it
//! is not computed from the log, so nothing rebuilds it.

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
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::{AdHocCopy, View};
use rusqlite::Connection;

use view_support::*;

/// The ad-hoc space, and a second one.
const AD_HOC: SpaceId = SpaceId([0xAD; 16]);
const OTHER_AD_HOC: SpaceId = SpaceId([0xAE; 16]);
/// Two source spaces.
const PROJECT: SpaceId = SpaceId([0x01; 16]);
const ARCHIVE: SpaceId = SpaceId([0x02; 16]);

fn object(byte: u8) -> ObjectId {
    ObjectId([byte; 16])
}

fn copy(copy: u8, source_space: SpaceId, source: u8) -> AdHocCopy {
    AdHocCopy {
        copy: object(copy),
        source_space,
        source: object(source),
    }
}

#[test]
fn an_object_that_is_no_copy_has_no_source() {
    let view = View::open_in_memory(&lock()).unwrap();
    assert_eq!(view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(), None);
    assert_eq!(
        view.ad_hoc_copy_of(AD_HOC, PROJECT, object(0x51)).unwrap(),
        None
    );
}

#[test]
fn each_copy_is_kept_with_its_source_both_ways() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.record_ad_hoc_copies(
        AD_HOC,
        &[copy(0xC1, PROJECT, 0x51), copy(0xC2, ARCHIVE, 0x52)],
    )
    .unwrap();

    assert_eq!(
        view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(),
        Some((PROJECT, object(0x51)))
    );
    assert_eq!(
        view.ad_hoc_source_of(AD_HOC, object(0xC2)).unwrap(),
        Some((ARCHIVE, object(0x52)))
    );
    assert_eq!(
        view.ad_hoc_copy_of(AD_HOC, PROJECT, object(0x51)).unwrap(),
        Some(object(0xC1))
    );
    assert_eq!(
        view.ad_hoc_copy_of(AD_HOC, ARCHIVE, object(0x52)).unwrap(),
        Some(object(0xC2))
    );
    // The source's id is not a copy, nor the source in another space.
    assert_eq!(view.ad_hoc_source_of(AD_HOC, object(0x51)).unwrap(), None);
    assert_eq!(
        view.ad_hoc_copy_of(AD_HOC, ARCHIVE, object(0x51)).unwrap(),
        None
    );
}

/// Each ad-hoc space has its own copies, also of one source.
#[test]
fn the_copies_of_one_ad_hoc_space_are_not_another_s() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();
    view.record_ad_hoc_copies(OTHER_AD_HOC, &[copy(0xD1, PROJECT, 0x51)])
        .unwrap();

    assert_eq!(
        view.ad_hoc_copy_of(AD_HOC, PROJECT, object(0x51)).unwrap(),
        Some(object(0xC1))
    );
    assert_eq!(
        view.ad_hoc_copy_of(OTHER_AD_HOC, PROJECT, object(0x51))
            .unwrap(),
        Some(object(0xD1))
    );
    assert_eq!(
        view.ad_hoc_source_of(OTHER_AD_HOC, object(0xC1)).unwrap(),
        None
    );
}

/// A copy recorded twice, or a second copy of one source in one ad-hoc
/// space, fails, and the batch it came in records nothing.
#[test]
fn a_batch_that_repeats_a_copy_or_a_source_records_nothing() {
    let mut view = View::open_in_memory(&lock()).unwrap();
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();

    let same_copy = [copy(0xC2, PROJECT, 0x52), copy(0xC1, ARCHIVE, 0x53)];
    assert!(view.record_ad_hoc_copies(AD_HOC, &same_copy).is_err());
    let same_source = [copy(0xC3, ARCHIVE, 0x54), copy(0xC4, PROJECT, 0x51)];
    assert!(view.record_ad_hoc_copies(AD_HOC, &same_source).is_err());

    for (space, source) in [(PROJECT, 0x52), (ARCHIVE, 0x53), (ARCHIVE, 0x54)] {
        assert_eq!(
            view.ad_hoc_copy_of(AD_HOC, space, object(source)).unwrap(),
            None
        );
    }
    assert_eq!(view.ad_hoc_source_of(AD_HOC, object(0xC2)).unwrap(), None);
    assert_eq!(
        view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(),
        Some((PROJECT, object(0x51)))
    );
}

#[test]
fn the_copies_survive_a_reopen() {
    let db = TempDb::new("ad-hoc-reopen");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();
    drop(view);
    let view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(
        view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(),
        Some((PROJECT, object(0x51)))
    );
}

/// The table's layout, part of the on-disk format (the owner's NOC-320 Q5:
/// the layout inside the core is the coding agent's call): the ad-hoc
/// space, the copy, the source's space and the source, nothing else.
#[test]
fn the_table_holds_the_space_the_copy_and_its_source_and_nothing_else() {
    let db = TempDb::new("ad-hoc-layout");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    let columns: Vec<(String, String, bool, i64)> = conn
        .prepare("SELECT name, type, \"notnull\", pk FROM pragma_table_info('ad_hoc_copies')")
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
            ("copy_id".to_owned(), "BLOB".to_owned(), true, 2),
            ("source_space_id".to_owned(), "BLOB".to_owned(), true, 0),
            ("source_id".to_owned(), "BLOB".to_owned(), true, 0),
        ]
    );
    let strict: i64 = conn
        .query_row(
            "SELECT strict FROM pragma_table_list WHERE name = 'ad_hoc_copies'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(strict, 1);
}

#[test]
fn a_copy_is_stored_as_four_16_byte_ids() {
    let db = TempDb::new("ad-hoc-row");
    let mut view = View::open(&db.0, &lock()).unwrap();
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();
    drop(view);
    let conn = Connection::open(&db.0).unwrap();
    let row: (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT space_id, copy_id, source_space_id, source_id FROM ad_hoc_copies",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            AD_HOC.0.to_vec(),
            object(0xC1).0.to_vec(),
            PROJECT.0.to_vec(),
            object(0x51).0.to_vec()
        )
    );
}

/// A database written before the table existed opens, gains the table and
/// keeps its format version: the table is additive, and a core that does
/// not know it ignores it.
#[test]
fn a_database_from_before_the_table_gains_it_and_keeps_its_format_version() {
    let db = TempDb::new("ad-hoc-before");
    drop(View::open(&db.0, &lock()).unwrap());
    let conn = Connection::open(&db.0).unwrap();
    conn.execute_batch("DROP TABLE ad_hoc_copies").unwrap();
    drop(conn);

    let mut view = View::open(&db.0, &lock()).unwrap();
    assert_eq!(view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(), None);
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();
    drop(view);

    let conn = Connection::open(&db.0).unwrap();
    let format: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'format'", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(format, View::FORMAT.to_string());
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
/// tables; the copies are no part of the view and stay.
#[test]
fn folding_the_log_again_keeps_the_copies() {
    let db = TempDb::new("ad-hoc-refold");
    let mut view = View::open_with(&db.0, &older(), checks_for).unwrap();
    view.record_ad_hoc_copies(AD_HOC, &[copy(0xC1, PROJECT, 0x51)])
        .unwrap();
    drop(view);

    let view = View::open_with(&db.0, &lock(), checks_for).unwrap();
    assert!(view.model().collections[TASK].contains_key(RANK));
    assert_eq!(
        view.ad_hoc_source_of(AD_HOC, object(0xC1)).unwrap(),
        Some((PROJECT, object(0x51)))
    );
}
