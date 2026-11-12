//! The database on OPFS in a dedicated worker (NOC-175; RFC 0006, "The
//! browser", as the owner decided NOC-231): its name, its place on OPFS,
//! its keys and how durable it is. The name checks and the refusal outside
//! a worker run in Node, in the gates; Node has neither OPFS nor a worker,
//! so the rest is ignored there, and the `sdk-keys` step of
//! browser-view.yml runs it in Chromium.

use std::path::Path;

use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::View;
use ncdb_core::view::browser::{Storage as Vfs, exists, install};
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::{Placed, place, valid_name};
use crate::durability::Storage;
use crate::error::ErrorCode;
use crate::keys::Keys;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// A database name no other test uses.
fn fresh(what: &str) -> String {
    let random = crate::keys::random();
    let tag: String = random[..6].iter().map(|b| format!("{b:02x}")).collect();
    format!("{what}-{tag}")
}

/// Whether `a` and `b` are the same keys.
fn same(a: &Keys, b: &Keys) -> bool {
    *a.encode() == *b.encode()
}

/// Opens the view at the place's path and drops it, as a first open does.
fn open_view(placed: &Placed) {
    let lock = ModelLock::from_json(LOCK).unwrap();
    drop(View::open(Path::new(&placed.path), &lock).unwrap());
}

#[test]
fn a_name_is_1_to_64_characters_of_lowercase_letters_digits_dash_and_underscore() {
    let longest = "a".repeat(64);
    for name in ["default", "a", "my-db_2", "0", longest.as_str()] {
        assert!(valid_name(name).is_ok(), "{name:?} is refused");
    }
    let too_long = "a".repeat(65);
    for name in ["", too_long.as_str(), "A", "a/b", "..", "a b", "ü", "a.b"] {
        let refused = valid_name(name).expect_err(name);
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{name:?}");
    }
}

#[test]
async fn a_database_at_a_name_is_refused_outside_a_dedicated_worker() {
    // Node is no dedicated worker: OPFS's synchronous handles exist only there.
    let worker = js_sys::Reflect::has(&js_sys::global(), &"DedicatedWorkerGlobalScope".into());
    if worker.unwrap_or(false) {
        return;
    }
    let refused = place("default").await.err().expect("refused");
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
async fn a_name_that_is_not_allowed_is_refused_before_anything_opens() {
    let refused = place("Not/Allowed").await.err().expect("refused");
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_a_database_file_exists_once_a_view_was_opened_there() {
    assert_eq!(install().await.unwrap(), Vfs::Opfs);
    let path = format!("/{}/nocentraldb.sqlite", fresh("exists"));
    assert!(!exists(&path).await.unwrap());

    let lock = ModelLock::from_json(LOCK).unwrap();
    drop(View::open(Path::new(&path), &lock).unwrap());

    assert!(exists(&path).await.unwrap());
    assert!(
        !exists(&format!("/{}/nocentraldb.sqlite", fresh("never")))
            .await
            .unwrap()
    );
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_a_database_lives_at_its_name_and_keeps_its_keys_across_opens() {
    let name = fresh("reopen");

    let first = place(&name).await.unwrap();
    assert_eq!(first.path, format!("/{name}/nocentraldb.sqlite"));
    open_view(&first);
    let second = place(&name).await.unwrap();

    assert!(same(&first.keys, &second.keys));
    assert_eq!(second.path, first.path);
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_a_copy_on_opfs_without_persistent_storage_is_best_effort() {
    // Playwright's Chromium does not grant persistent storage (NOC-33).
    let placed = place(&fresh("durable")).await.unwrap();

    assert_eq!(placed.storage, Storage::BestEffort);
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_two_names_are_two_databases_with_their_own_keys() {
    let one = place(&fresh("one")).await.unwrap();
    let two = place(&fresh("two")).await.unwrap();

    assert_ne!(one.path, two.path);
    assert!(!same(&one.keys, &two.keys));
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_a_database_whose_keys_are_gone_does_not_open_and_draws_none() {
    let name = fresh("keyless");
    let placed = place(&name).await.unwrap();
    open_view(&placed);
    let keys = crate::keys::browser::indexeddb::IndexedDb::open(&name)
        .await
        .unwrap();
    keys.delete("key-record").await.unwrap();
    keys.delete("wrapping-key").await.unwrap();
    drop(keys);

    let refused = place(&name).await.err().expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
}

#[test]
#[ignore = "needs OPFS in a dedicated worker: the sdk-keys step of browser-view.yml runs it in Chromium"]
async fn opfs_each_database_reserves_its_two_slots_in_the_pool() {
    // The pool starts with six slots; four databases with their journals
    // need eight.
    for n in 0..4 {
        let placed = place(&fresh(&format!("slots{n}"))).await.unwrap();
        open_view(&placed);
    }
}
