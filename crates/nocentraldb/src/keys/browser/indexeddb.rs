//! The two records of a database's keys in IndexedDB (NOC-210 Q1 A): the
//! database `nocentraldb/<name>`, version 1, with the object store `keys`,
//! which holds the wrapping key under `wrapping-key`, as the `CryptoKey`
//! object itself, and the sealed key record under `key-record`, as an
//! `ArrayBuffer`.

use js_sys::{Reflect, Uint8Array};
use tokio::sync::oneshot;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    CryptoKey, DomException, Event, IdbDatabase, IdbFactory, IdbObjectStore, IdbOpenDbRequest,
    IdbRequest, IdbTransaction, IdbTransactionMode,
};

use super::{Records, Stored};
use crate::error::{Error, ErrorCode};

/// What the IndexedDB database's name starts with.
const PREFIX: &str = "nocentraldb/";
/// The version of the IndexedDB database.
const VERSION: u32 = 1;
/// The object store that holds the two records.
const STORE: &str = "keys";
/// The key of the wrapping key.
const WRAPPING_KEY: &str = "wrapping-key";
/// The key of the sealed key record.
const KEY_RECORD: &str = "key-record";

/// The IndexedDB database of one SDK database's keys, open.
pub(crate) struct IndexedDb {
    db: IdbDatabase,
}

impl IndexedDb {
    /// Opens the IndexedDB database of the SDK database `name`, creating it
    /// and its object store when it does not exist.
    pub(crate) async fn open(name: &str) -> Result<IndexedDb, Error> {
        let factory = Reflect::get(&js_sys::global(), &JsValue::from_str("indexedDB"))
            .ok()
            .filter(|factory| factory.is_object())
            .ok_or_else(|| {
                Error::new(
                    ErrorCode::KeyStoreUnavailable,
                    "this browser context has no IndexedDB, which keeps the member's keys",
                )
            })?
            .unchecked_into::<IdbFactory>();
        let request = factory
            .open_with_u32(&format!("{PREFIX}{name}"), VERSION)
            .map_err(|error| refused("IndexedDB cannot be opened", &error))?;
        let upgrade = Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let db = event
                .target()
                .and_then(|target| target.dyn_into::<IdbOpenDbRequest>().ok())
                .and_then(|request| request.result().ok())
                .map(JsValue::unchecked_into::<IdbDatabase>);
            if let Some(db) = db {
                // Version 1 is the first, so its upgrade creates the store.
                let _ = db.create_object_store(STORE);
            }
        });
        request.set_onupgradeneeded(Some(upgrade.as_ref().unchecked_ref()));
        let db = done(&request).await;
        request.set_onupgradeneeded(None);
        let db = db.map_err(|error| refused("IndexedDB cannot be opened", &error))?;
        Ok(IndexedDb {
            db: db.unchecked_into(),
        })
    }

    /// The IndexedDB database's name.
    #[cfg(test)]
    pub(crate) fn name(&self) -> String {
        self.db.name()
    }

    /// The value under `key`, `undefined` when there is none.
    pub(crate) async fn get(&self, key: &str) -> Result<JsValue, Error> {
        let (_, store) = self.store(IdbTransactionMode::Readonly)?;
        let request = store
            .get(&JsValue::from_str(key))
            .map_err(|error| refused("IndexedDB cannot be read", &error))?;
        done(&request)
            .await
            .map_err(|error| refused("IndexedDB cannot be read", &error))
    }

    /// Deletes the value under `key`.
    #[cfg(test)]
    pub(crate) async fn delete(&self, key: &str) -> Result<(), Error> {
        let (transaction, store) = self.store(IdbTransactionMode::Readwrite)?;
        store
            .delete(&JsValue::from_str(key))
            .map_err(|error| refused("IndexedDB cannot be written", &error))?;
        complete(&transaction)
            .await
            .map_err(|error| refused("IndexedDB cannot be written", &error))
    }

    /// The keys the object store holds, sorted.
    #[cfg(test)]
    pub(crate) async fn keys(&self) -> Result<Vec<String>, Error> {
        let (_, store) = self.store(IdbTransactionMode::Readonly)?;
        let request = store
            .get_all_keys()
            .map_err(|error| refused("IndexedDB cannot be read", &error))?;
        let keys = done(&request)
            .await
            .map_err(|error| refused("IndexedDB cannot be read", &error))?;
        let mut keys: Vec<String> = js_sys::Array::from(&keys)
            .iter()
            .filter_map(|key| key.as_string())
            .collect();
        keys.sort();
        Ok(keys)
    }

    /// A transaction on the object store, in `mode`, and the store.
    fn store(&self, mode: IdbTransactionMode) -> Result<(IdbTransaction, IdbObjectStore), Error> {
        let transaction = self
            .db
            .transaction_with_str_and_mode(STORE, mode)
            .map_err(|error| refused("IndexedDB cannot be used", &error))?;
        let store = transaction
            .object_store(STORE)
            .map_err(|error| refused("IndexedDB cannot be used", &error))?;
        Ok((transaction, store))
    }
}

impl Drop for IndexedDb {
    fn drop(&mut self) {
        self.db.close();
    }
}

impl Records for IndexedDb {
    async fn read(&self) -> Result<Stored, Error> {
        let wrapping_key = self.get(WRAPPING_KEY).await?;
        let record = self.get(KEY_RECORD).await?;
        Ok(Stored {
            wrapping_key: (!wrapping_key.is_undefined()).then(|| wrapping_key.unchecked_into()),
            record: (!record.is_undefined()).then(|| Uint8Array::new(&record).to_vec()),
        })
    }

    async fn write(&self, wrapping_key: &CryptoKey, record: &[u8]) -> Result<(), Error> {
        let (transaction, store) = self.store(IdbTransactionMode::Readwrite)?;
        // `add`, not `put`: keys another context kept meanwhile are never
        // overwritten; the transaction then fails whole.
        let buffer = Uint8Array::from(record).buffer();
        store
            .add_with_key(wrapping_key, &JsValue::from_str(WRAPPING_KEY))
            .and_then(|_| store.add_with_key(&buffer, &JsValue::from_str(KEY_RECORD)))
            .map_err(|error| refused("IndexedDB cannot be written", &error))?;
        complete(&transaction)
            .await
            .map_err(|error| refused("IndexedDB cannot be written", &error))
    }
}

/// What `request` gave, once it succeeded, or its error.
async fn done(request: &IdbRequest) -> Result<JsValue, JsValue> {
    let (sender, receiver) = oneshot::channel();
    let sender = std::rc::Rc::new(std::cell::Cell::new(Some(sender)));
    let (ok, failed) = (sender.clone(), sender);
    let on_success = {
        let request = request.clone();
        Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            if let Some(sender) = ok.take() {
                let _ = sender.send(request.result());
            }
        })
    };
    let on_error = {
        let request = request.clone();
        Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            if let Some(sender) = failed.take() {
                let error = request.error().ok().flatten().map(JsValue::from);
                let _ = sender.send(Err(error.unwrap_or(JsValue::UNDEFINED)));
            }
        })
    };
    request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
    request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    let result = receiver.await.unwrap_or(Err(JsValue::UNDEFINED));
    request.set_onsuccess(None);
    request.set_onerror(None);
    result
}

/// Waits until `transaction` completes, or its error once it fails or
/// aborts.
async fn complete(transaction: &IdbTransaction) -> Result<(), JsValue> {
    let (sender, receiver) = oneshot::channel();
    let sender = std::rc::Rc::new(std::cell::Cell::new(Some(sender)));
    let (ok, failed) = (sender.clone(), sender);
    let on_complete = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
        if let Some(sender) = ok.take() {
            let _ = sender.send(Ok(()));
        }
    });
    let on_failure = {
        let transaction = transaction.clone();
        Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            if let Some(sender) = failed.take() {
                let error = transaction.error().map(JsValue::from);
                let _ = sender.send(Err(error.unwrap_or(JsValue::UNDEFINED)));
            }
        })
    };
    transaction.set_oncomplete(Some(on_complete.as_ref().unchecked_ref()));
    transaction.set_onerror(Some(on_failure.as_ref().unchecked_ref()));
    transaction.set_onabort(Some(on_failure.as_ref().unchecked_ref()));
    let result = receiver.await.unwrap_or(Err(JsValue::UNDEFINED));
    transaction.set_oncomplete(None);
    transaction.set_onerror(None);
    transaction.set_onabort(None);
    result
}

/// An IndexedDB failure: `StorageFull` when the quota is exhausted,
/// `KeyStoreUnavailable` when the browser refuses IndexedDB, as in some
/// private windows, `StorageFailed` otherwise. The message names the
/// failure, never a value.
fn refused(what: &str, error: &JsValue) -> Error {
    let name = error_name(error);
    let code = match name.as_str() {
        "QuotaExceededError" => ErrorCode::StorageFull,
        "SecurityError" | "InvalidAccessError" => ErrorCode::KeyStoreUnavailable,
        _ => ErrorCode::StorageFailed,
    };
    let hint = match name.as_str() {
        "ConstraintError" => {
            "; another context kept keys for this database meanwhile: open it again"
        }
        _ => "",
    };
    Error::new(code, format!("{what}: {name}{hint}"))
}

/// The name of a thrown JavaScript error, such as `QuotaExceededError`.
pub(crate) fn error_name(error: &JsValue) -> String {
    error
        .dyn_ref::<DomException>()
        .map(DomException::name)
        .or_else(|| {
            Reflect::get(error, &JsValue::from_str("name"))
                .ok()
                .and_then(|name| name.as_string())
        })
        .unwrap_or_else(|| "an unknown error".to_owned())
}
