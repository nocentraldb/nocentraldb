//! The member's keys in the browser (NOC-175 slice 2; NOC-210 A): the key
//! record sealed with AES-GCM under a non-extractable WebCrypto key, both
//! kept in IndexedDB.
//!
//! The tests of the seal and of what an open does with the two records run
//! in Node, in the gates, through WebCrypto and an in-memory store. Node has
//! no IndexedDB, so the tests of the IndexedDB store are ignored there; the
//! `sdk-keys` job of browser-view.yml runs them in a dedicated worker in
//! Chromium (`--ignored`).

use std::cell::{Cell, RefCell};

use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;
use web_sys::CryptoKey;

use super::indexeddb::IndexedDb;
use super::{Records, SEALED_LEN, Stored, from_records, generate, load_or_create, open, seal};
use crate::error::{Error, ErrorCode};
use crate::keys::Keys;

/// The two records in memory, in place of IndexedDB, counting the writes.
#[derive(Default)]
struct Memory {
    wrapping_key: RefCell<Option<CryptoKey>>,
    record: RefCell<Option<Vec<u8>>>,
    writes: Cell<u32>,
}

impl Records for Memory {
    async fn read(&self) -> Result<Stored, Error> {
        Ok(Stored {
            wrapping_key: self.wrapping_key.borrow().clone(),
            record: self.record.borrow().clone(),
        })
    }

    async fn write(&self, wrapping_key: &CryptoKey, record: &[u8]) -> Result<(), Error> {
        *self.wrapping_key.borrow_mut() = Some(wrapping_key.clone());
        *self.record.borrow_mut() = Some(record.to_vec());
        self.writes.set(self.writes.get() + 1);
        Ok(())
    }
}

/// Whether `a` and `b` are the same keys.
fn same(a: &Keys, b: &Keys) -> bool {
    *a.encode() == *b.encode()
}

/// A database name no other test uses, so IndexedDB starts empty for it.
fn fresh(what: &str) -> String {
    let random = crate::keys::random();
    let tag: String = random[..8].iter().map(|b| format!("{b:02x}")).collect();
    format!("{what}-{tag}")
}

#[test]
async fn a_sealed_record_opens_under_its_wrapping_key_and_database_name() {
    let key = generate().await.unwrap();
    let record = Keys::generate().encode();

    let sealed = seal(&key, "default", &record).await.unwrap();

    assert_eq!(sealed.len(), SEALED_LEN);
    assert_eq!(SEALED_LEN, 12 + 113 + 16, "IV, the key record, the tag");
    let opened = open(&key, "default", &sealed).await.expect("it opens");
    assert_eq!(*opened, *record);
}

#[test]
async fn a_sealed_record_does_not_open_under_another_database_name() {
    let key = generate().await.unwrap();
    let sealed = seal(&key, "default", &Keys::generate().encode())
        .await
        .unwrap();

    assert!(open(&key, "other", &sealed).await.is_none());
    assert!(open(&key, "defaul", &sealed).await.is_none());
}

#[test]
async fn a_sealed_record_does_not_open_under_another_wrapping_key() {
    let key = generate().await.unwrap();
    let sealed = seal(&key, "default", &Keys::generate().encode())
        .await
        .unwrap();

    assert!(
        open(&generate().await.unwrap(), "default", &sealed)
            .await
            .is_none()
    );
}

#[test]
async fn a_sealed_record_with_one_byte_changed_does_not_open() {
    let key = generate().await.unwrap();
    let sealed = seal(&key, "default", &Keys::generate().encode())
        .await
        .unwrap();
    // A byte of the IV, of the ciphertext and of the tag.
    for at in [0, 12, 60, SEALED_LEN - 1] {
        let mut changed = sealed.clone();
        changed[at] ^= 0x01;
        assert!(open(&key, "default", &changed).await.is_none(), "byte {at}");
    }
    assert!(
        open(&key, "default", &sealed[..SEALED_LEN - 1])
            .await
            .is_none()
    );
}

#[test]
async fn two_seals_of_one_record_differ_by_their_random_iv() {
    let key = generate().await.unwrap();
    let record = Keys::generate().encode();

    let first = seal(&key, "default", &record).await.unwrap();
    let second = seal(&key, "default", &record).await.unwrap();

    assert_ne!(first[..12], second[..12]);
    assert_ne!(first, second);
}

#[test]
async fn the_wrapping_key_is_aes_gcm_256_for_encrypt_and_decrypt_and_cannot_be_extracted() {
    let key = generate().await.unwrap();

    assert!(!key.extractable());
    let algorithm = key.algorithm().unwrap();
    let get = |field: &str| js_sys::Reflect::get(&algorithm, &JsValue::from_str(field)).unwrap();
    assert_eq!(get("name").as_string().as_deref(), Some("AES-GCM"));
    assert_eq!(get("length").as_f64(), Some(256.0));
    let mut usages: Vec<String> = js_sys::Array::from(&key.usages())
        .iter()
        .filter_map(|usage| usage.as_string())
        .collect();
    usages.sort();
    assert_eq!(usages, ["decrypt", "encrypt"]);
}

#[test]
async fn the_first_open_draws_keys_and_keeps_them_sealed_under_a_new_wrapping_key() {
    let store = Memory::default();

    let keys = from_records(&store, "default", false).await.unwrap();

    assert_eq!(store.writes.get(), 1);
    let key = store.wrapping_key.borrow().clone().expect("a wrapping key");
    let record = store.record.borrow().clone().expect("a record");
    assert_eq!(record.len(), SEALED_LEN);
    let opened = open(&key, "default", &record).await.expect("it opens");
    assert_eq!(*opened, *keys.encode());
}

#[test]
async fn a_second_open_reads_the_same_keys_and_writes_nothing() {
    let store = Memory::default();
    let first = from_records(&store, "default", false).await.unwrap();

    let second = from_records(&store, "default", true).await.unwrap();

    assert!(same(&first, &second));
    assert_eq!(store.writes.get(), 1);
}

/// A database whose file exists, but whose keys are gone, as after the
/// browser cleared IndexedDB alone: new keys would open none of its spaces
/// (NOC-175; the native keystore's rule, `keys::store::from_store`).
#[test]
async fn a_database_that_exists_without_its_keys_fails_and_draws_no_keys() {
    let store = Memory::default();

    let refused = from_records(&store, "default", true)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 0);
}

#[test]
async fn a_wrapping_key_without_its_record_fails_and_draws_no_keys() {
    let store = Memory::default();
    *store.wrapping_key.borrow_mut() = Some(generate().await.unwrap());

    let refused = from_records(&store, "default", false)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 0);
}

#[test]
async fn a_record_without_its_wrapping_key_fails_and_draws_no_keys() {
    let store = Memory::default();
    let key = generate().await.unwrap();
    *store.record.borrow_mut() = Some(
        seal(&key, "default", &Keys::generate().encode())
            .await
            .unwrap(),
    );

    let refused = from_records(&store, "default", false)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 0);
}

#[test]
async fn a_record_that_does_not_open_fails_and_draws_no_keys() {
    let store = Memory::default();
    let other = generate().await.unwrap();
    *store.wrapping_key.borrow_mut() = Some(generate().await.unwrap());
    *store.record.borrow_mut() = Some(
        seal(&other, "default", &Keys::generate().encode())
            .await
            .unwrap(),
    );

    let refused = from_records(&store, "default", false)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 0);
}

#[test]
async fn the_records_of_another_database_name_do_not_open() {
    let store = Memory::default();
    from_records(&store, "default", false).await.unwrap();

    let refused = from_records(&store, "other", true)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 1);
}

#[test]
async fn a_record_that_opens_to_another_format_fails() {
    let store = Memory::default();
    let key = generate().await.unwrap();
    *store.record.borrow_mut() = Some(seal(&key, "default", &[0; 113]).await.unwrap());
    *store.wrapping_key.borrow_mut() = Some(key);

    let refused = from_records(&store, "default", false)
        .await
        .err()
        .expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(store.writes.get(), 0);
}

#[test]
#[ignore = "needs IndexedDB: the sdk-keys job of browser-view.yml runs it in Chromium"]
async fn indexeddb_a_database_opened_twice_reads_the_same_keys() {
    let name = fresh("twice");

    let first = load_or_create(&name, false).await.unwrap();
    let second = load_or_create(&name, true).await.unwrap();

    assert!(same(&first, &second));
}

#[test]
#[ignore = "needs IndexedDB: the sdk-keys job of browser-view.yml runs it in Chromium"]
async fn indexeddb_holds_the_wrapping_key_and_the_sealed_record_under_their_names() {
    let name = fresh("layout");
    let keys = load_or_create(&name, false).await.unwrap();

    // The layout NOC-210 A gives: the database `nocentraldb/<name>`, its
    // object store `keys`, and the two records under these keys.
    let db = IndexedDb::open(&name).await.unwrap();
    assert_eq!(db.name(), format!("nocentraldb/{name}"));
    let key: CryptoKey = db.get("wrapping-key").await.unwrap().into();
    assert!(!key.extractable());
    let record = js_sys::Uint8Array::new(&db.get("key-record").await.unwrap()).to_vec();
    assert_eq!(record.len(), SEALED_LEN);
    let opened = open(&key, &name, &record).await.expect("it opens");
    assert_eq!(*opened, *keys.encode());
    assert_eq!(db.keys().await.unwrap(), ["key-record", "wrapping-key"]);
}

#[test]
#[ignore = "needs IndexedDB: the sdk-keys job of browser-view.yml runs it in Chromium"]
async fn indexeddb_a_database_whose_record_is_gone_does_not_draw_new_keys() {
    let name = fresh("gone");
    load_or_create(&name, false).await.unwrap();
    IndexedDb::open(&name)
        .await
        .unwrap()
        .delete("key-record")
        .await
        .unwrap();

    let refused = load_or_create(&name, true).await.err().expect("refused");

    assert_eq!(refused.code(), ErrorCode::StorageFailed);
    assert_eq!(
        IndexedDb::open(&name).await.unwrap().keys().await.unwrap(),
        ["wrapping-key"]
    );
}

#[test]
#[ignore = "needs IndexedDB: the sdk-keys job of browser-view.yml runs it in Chromium"]
async fn indexeddb_two_database_names_hold_different_keys() {
    let first = load_or_create(&fresh("one"), false).await.unwrap();
    let second = load_or_create(&fresh("two"), false).await.unwrap();

    assert!(!same(&first, &second));
}
