//! One SQLite release on every target (NOC-174; RFC 0006, "The same fold
//! code on every target"): a difference in SQLite version is a difference
//! in behaviour, so the SQLite that `libsqlite3-sys` builds natively and
//! the one `sqlite-wasm-rs` builds for wasm32 must be the same release,
//! from the same amalgamation. This test runs on both targets and asks
//! SQLite itself; the browser test (`scripts/browser-view/`) asks the
//! SQLite in a browser's worker the same question.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use rusqlite::Connection;

/// The SQLite release both targets build: `libsqlite3-sys` 0.39.1 natively
/// and `sqlite-wasm-rs` 0.6.3 on wasm32, as `crates/ncdb-core/Cargo.toml`
/// pins them.
const VERSION: &str = "3.54.1";
/// The check-in of that release, which tells two builds of one version
/// from different sources apart.
const SOURCE_ID: &str =
    "2026-10-16 14:45:20 555e07ef33f69a0957d24d8e555b702464a131d6f1607f4c3c1e4551e8e6ca76";

#[test]
fn sqlite_reports_the_one_release_both_targets_build() {
    let conn = Connection::open_in_memory().unwrap();
    let (version, source_id): (String, String) = conn
        .query_row("SELECT sqlite_version(), sqlite_source_id()", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(version, VERSION);
    assert_eq!(source_id, SOURCE_ID);
}

#[test]
fn the_library_linked_in_is_the_release_sqlite_reports() {
    assert_eq!(rusqlite::version(), VERSION);
}
