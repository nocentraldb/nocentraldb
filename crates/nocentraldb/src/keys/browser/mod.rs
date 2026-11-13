//! The member's keys in the browser (RFC 0003, "The member key"; NOC-175
//! slice 2; the owner's NOC-210 Q1 A): the key record, sealed with
//! AES-GCM-256 under a non-extractable WebCrypto key, and that key, both
//! kept in IndexedDB ([`indexeddb`]).
//!
//! The sealed record is a 12-byte random IV, then the AES-GCM ciphertext of
//! the key record and its 16-byte tag, 28 bytes more than the record; its
//! associated data is `"nocentraldb/browser-key-record"` followed by the
//! database name's UTF-8 bytes, so the records of one database do not open
//! as another's. An open that finds neither record draws new keys and
//! keeps both, unless the database already exists; one that finds only one
//! of them, a record that does not open to a key record of this SDK's
//! format, or neither record for a database that exists, fails with
//! `StorageFailed` and never draws new keys over it, as a keystore entry
//! that is gone does natively ([`super::store::from_store`]).
//!
//! WebCrypto hands the opened record to the JavaScript engine as an
//! `ArrayBuffer` before the SDK copies it into its own memory; the SDK then
//! zeroes that buffer. The record never reaches the app, but it sits
//! briefly in the engine's memory, which RFC 0003 states.
//!
//! **What this does not do yet.** Nothing opens a database at a name on
//! wasm32: the database on OPFS and the browser export call
//! [`load_or_create`] when they are built (NOC-175, NOC-176). No passkey
//! unlocks the wrapping key (NOC-210 Q1 A).

pub(crate) mod indexeddb;
#[cfg(test)]
mod tests;

use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AesGcmParams, AesKeyGenParams, Crypto, CryptoKey, SubtleCrypto};
use zeroize::Zeroizing;

use super::Keys;
use crate::error::{Error, ErrorCode};

/// What the associated data of a sealed record starts with.
const AD: &[u8] = b"nocentraldb/browser-key-record";
/// The length of a sealed record's IV.
const IV_LEN: usize = 12;
/// The length of an AES-GCM tag.
const TAG_LEN: usize = 16;
/// What a store holds of a database's two records.
pub(crate) struct Stored {
    /// The wrapping key.
    pub(crate) wrapping_key: Option<CryptoKey>,
    /// The sealed key record.
    pub(crate) record: Option<Vec<u8>>,
}

/// Where the two records of one database are kept: IndexedDB in the
/// browser ([`indexeddb::IndexedDb`]), memory in the tests.
#[allow(async_fn_in_trait)]
pub(crate) trait Records {
    /// Both records, each when it is there.
    async fn read(&self) -> Result<Stored, Error>;
    /// Keeps both records at once, and fails when either is there already.
    async fn write(&self, wrapping_key: &CryptoKey, record: &[u8]) -> Result<(), Error>;
    /// Replaces the sealed key record, keeping the wrapping key.
    async fn replace(&self, record: &[u8]) -> Result<(), Error>;
}

/// The keys of the database `name`, from IndexedDB: `new` keys, kept
/// there, when it holds none for that name and the database did not exist.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) async fn load_or_create(
    name: &str,
    database_existed: bool,
    new: impl FnOnce() -> Keys,
) -> Result<Keys, Error> {
    let db = indexeddb::IndexedDb::open(name).await?;
    from_records(&db, name, database_existed, new).await
}

/// Keeps `keys` as the key record of the database `name` in IndexedDB,
/// sealed under the wrapping key it holds.
pub(crate) async fn save(name: &str, keys: &Keys) -> Result<(), Error> {
    let db = indexeddb::IndexedDb::open(name).await?;
    replace(&db, name, keys).await
}

/// Seals `keys` under the wrapping key `records` holds for the database
/// `name`, and replaces its key record.
pub(crate) async fn replace(records: &impl Records, name: &str, keys: &Keys) -> Result<(), Error> {
    let wrapping_key = records
        .read()
        .await?
        .wrapping_key
        .ok_or_else(|| failed("IndexedDB holds no wrapping key for the database"))?;
    let sealed = seal(&wrapping_key, name, &keys.encode()).await?;
    records.replace(&sealed).await
}

/// The keys of the database `name` from `records`, or `new` keys, sealed
/// and kept there, when it holds neither record and the database did not
/// exist. A record of format 1 is kept again as format 2.
pub(crate) async fn from_records(
    records: &impl Records,
    name: &str,
    database_existed: bool,
    new: impl FnOnce() -> Keys,
) -> Result<Keys, Error> {
    let Stored {
        wrapping_key,
        record,
    } = records.read().await?;
    match (wrapping_key, record) {
        (None, None) if database_existed => Err(failed(
            "the database exists, but IndexedDB holds no keys for it; new keys would open none of its spaces",
        )),
        (None, None) => {
            let keys = new();
            let wrapping_key = generate().await?;
            let sealed = seal(&wrapping_key, name, &keys.encode()).await?;
            records.write(&wrapping_key, &sealed).await?;
            Ok(keys)
        }
        (Some(wrapping_key), Some(record)) => {
            let opened = open(&wrapping_key, name, &record).await.ok_or_else(|| {
                failed("the key record of the database does not open under its wrapping key")
            })?;
            let keys = Keys::decode(&opened).ok_or_else(|| {
                failed("the key record of the database holds no keys of this SDK's format")
            })?;
            if keys.upgraded {
                let sealed = seal(&wrapping_key, name, &keys.encode()).await?;
                records.replace(&sealed).await?;
            }
            Ok(keys)
        }
        _ => Err(failed(
            "IndexedDB holds only one of the database's two key records; new keys would open none of its spaces",
        )),
    }
}

/// A new wrapping key: AES-GCM-256, for encrypt and decrypt, which cannot
/// be extracted.
pub(crate) async fn generate() -> Result<CryptoKey, Error> {
    let params = AesKeyGenParams::new("AES-GCM", 256);
    let usages = Array::of2(&"encrypt".into(), &"decrypt".into());
    let promise = subtle()?
        .generate_key_with_object(&params, false, &usages)
        .map_err(|error| crypto("the wrapping key cannot be made", &error))?;
    let key = JsFuture::from(promise)
        .await
        .map_err(|error| crypto("the wrapping key cannot be made", &error))?;
    Ok(key.unchecked_into())
}

/// `record` sealed under `key` for the database `name`, with a fresh
/// random IV.
pub(crate) async fn seal(key: &CryptoKey, name: &str, record: &[u8]) -> Result<Vec<u8>, Error> {
    let mut iv = [0; IV_LEN];
    getrandom::fill(&mut iv).expect("the platform's secure random generator failed");
    let promise = subtle()?
        .encrypt_with_object_and_u8_array(&params(&iv, name), key, record)
        .map_err(|error| crypto("the key record cannot be sealed", &error))?;
    let sealed = JsFuture::from(promise)
        .await
        .map_err(|error| crypto("the key record cannot be sealed", &error))?;
    let mut out = iv.to_vec();
    out.extend(Uint8Array::new(&sealed).to_vec());
    Ok(out)
}

/// The record `sealed` holds, when it opens under `key` for the database
/// `name`. The engine's copy of it is zeroed.
pub(crate) async fn open(key: &CryptoKey, name: &str, sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    if sealed.len() < IV_LEN + TAG_LEN {
        return None;
    }
    let (iv, body) = sealed.split_at(IV_LEN);
    let promise = subtle()
        .ok()?
        .decrypt_with_object_and_u8_array(&params(iv, name), key, body)
        .ok()?;
    let opened = Uint8Array::new(&JsFuture::from(promise).await.ok()?);
    let mut record = Zeroizing::new(vec![0; opened.length() as usize]);
    opened.copy_to(&mut record);
    opened.fill(0, 0, opened.length());
    Some(record)
}

/// The AES-GCM parameters of a seal: the IV, and the associated data of
/// the database `name`.
fn params(iv: &[u8], name: &str) -> AesGcmParams {
    let params = AesGcmParams::new("AES-GCM", &Uint8Array::from(iv));
    let mut ad = AD.to_vec();
    ad.extend_from_slice(name.as_bytes());
    params.set_additional_data_u8_array(&Uint8Array::from(ad.as_slice()));
    params
}

/// The global's WebCrypto, in a window, a worker or Node.
fn subtle() -> Result<SubtleCrypto, Error> {
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .ok()
        .filter(|crypto| crypto.is_object())
        .ok_or_else(|| {
            Error::new(
                ErrorCode::KeyStoreUnavailable,
                "this browser context has no WebCrypto, which keeps the member's keys",
            )
        })?;
    Ok(crypto.unchecked_into::<Crypto>().subtle())
}

/// `StorageFailed` with `what`.
fn failed(what: &str) -> Error {
    Error::new(ErrorCode::StorageFailed, what)
}

/// A WebCrypto failure: `what`, and the name of the error it threw, never
/// any byte.
fn crypto(what: &str, error: &JsValue) -> Error {
    failed(&format!("{what}: {}", indexeddb::error_name(error)))
}
