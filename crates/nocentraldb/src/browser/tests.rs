//! The database on OPFS in a dedicated worker (NOC-175). Node has neither
//! OPFS nor a worker, so these tests are ignored there; the `sdk-keys` step
//! of browser-view.yml runs them in Chromium, as `scripts/sdk-keys/` does
//! the keys' tests.

use std::path::Path;

use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;
use ncdb_core::view::browser::{Storage, exists, install};
use wasm_bindgen_test::wasm_bindgen_test as test;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// A file name no other test uses.
fn fresh(what: &str) -> String {
    let random = crate::keys::random();
    let tag: String = random[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("/{what}-{tag}.sqlite")
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_a_database_file_exists_once_a_view_was_opened_there() {
    assert_eq!(install().await.unwrap(), Storage::Opfs);
    let path = fresh("exists");
    assert!(!exists(&path).await.unwrap());

    let lock = ModelLock::from_json(LOCK).unwrap();
    drop(View::open(Path::new(&path), &lock).unwrap());

    assert!(exists(&path).await.unwrap());
    assert!(!exists(&fresh("never")).await.unwrap());
}
