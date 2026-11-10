//! A view error says whether the disk or the quota is full (NOC-88, after
//! the owner's decision 4 B on NOC-95): only then does the SDK report
//! `StorageFull`, which tells an app that freeing space would help; every
//! other failure of the database is `StorageFailed`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::view::Error;
use rusqlite::ffi;

fn sqlite(code: std::ffi::c_int) -> Error {
    Error::Sqlite(rusqlite::Error::SqliteFailure(ffi::Error::new(code), None))
}

#[test]
fn only_a_full_disk_or_quota_is_full() {
    assert!(sqlite(ffi::SQLITE_FULL).is_full());

    assert!(!sqlite(ffi::SQLITE_IOERR).is_full());
    assert!(!sqlite(ffi::SQLITE_CORRUPT).is_full());
    assert!(!Error::Corrupt("log").is_full());
    assert!(!Error::Model.is_full());
}
