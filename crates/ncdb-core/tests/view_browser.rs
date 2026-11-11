//! The view in a browser (RFC 0006, "The browser"; NOC-175): whether a
//! database file exists on OPFS, read from the `opfs-sahpool` VFS's files.
//! Outside a dedicated Web Worker, as in Node, it fails as `install` does;
//! the SDK's browser tests check it on OPFS in Chromium
//! (crates/nocentraldb/src/browser/tests.rs).

#![cfg(target_arch = "wasm32")]

use ncdb_core::view::browser::{InstallError, exists};
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
async fn outside_a_dedicated_worker_exists_fails_as_install_does() {
    assert!(matches!(
        exists("/view.sqlite").await,
        Err(InstallError::NotInWorker)
    ));
}
