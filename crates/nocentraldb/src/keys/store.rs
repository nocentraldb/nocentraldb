//! The keys in the platform keystore (RFC 0003, "How the SDK hides keys";
//! RFC 0009, S19 and S26): the Keychain on macOS, the Credential Manager on
//! Windows and the Secret Service on Linux, through `keyring-core`. One
//! entry holds a database's key record, under the service `nocentraldb`
//! and, as its user, the canonical path of the database's directory.
//!
//! A keystore that cannot be reached, or refuses access, is
//! `KeyStoreUnavailable`, whose message names `key_file`; the SDK never
//! falls back to a key file the app did not name (RFC 0003, decision 19).

use std::path::Path;
use std::sync::Arc;

use keyring_core::{CredentialStore, Error as KeyringError};
use zeroize::Zeroizing;

use super::Keys;
use crate::error::{Error, ErrorCode};

/// The service of every entry the SDK writes.
const SERVICE: &str = "nocentraldb";

/// Where a database's key record is kept, by the database's user name.
pub(crate) trait SecretStore {
    /// The record of `user`, or `None` when there is none.
    fn get(&self, user: &str) -> Result<Option<Zeroizing<Vec<u8>>>, Error>;
    /// Keeps `record` as the record of `user`.
    fn set(&self, user: &str, record: &[u8]) -> Result<(), Error>;
    /// Removes the record of `user`.
    #[cfg_attr(any(not(test), target_arch = "wasm32"), allow(dead_code))]
    fn delete(&self, user: &str) -> Result<(), Error>;
}

/// The keys of the database in `dir`, which must exist, from `store`. When
/// the store holds none and the database did not exist, new keys are drawn
/// and kept there. An existing database whose entry is gone fails with
/// `StorageFailed`: new keys would hold none of its spaces' keys.
pub(crate) fn from_store(
    store: &dyn SecretStore,
    dir: &Path,
    database_existed: bool,
) -> Result<Keys, Error> {
    let dir = dir
        .canonicalize()
        .map_err(|error| super::io("the database directory cannot be found", &error))?;
    let user = dir.to_string_lossy();
    match store.get(&user)? {
        Some(record) => Keys::decode(&record).ok_or_else(|| {
            Error::new(
                ErrorCode::StorageFailed,
                "the keystore entry of the database holds no keys of this SDK's format",
            )
        }),
        None if database_existed => Err(Error::new(
            ErrorCode::StorageFailed,
            "the database exists, but the keystore holds no keys for it; new keys would open none of its spaces",
        )),
        None => {
            let keys = Keys::generate();
            store.set(&user, &keys.encode())?;
            Ok(keys)
        }
    }
}

/// The platform's keystore, or the keystore's error when none is
/// reachable, which [`unavailable`] turns into the SDK's.
pub(crate) fn platform() -> Result<Box<dyn SecretStore>, KeyringError> {
    Ok(Box::new(Platform(store()?)))
}

#[cfg(target_os = "macos")]
fn store() -> Result<Arc<CredentialStore>, KeyringError> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(target_os = "windows")]
fn store() -> Result<Arc<CredentialStore>, KeyringError> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn store() -> Result<Arc<CredentialStore>, KeyringError> {
    Ok(zbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    )
)))]
fn store() -> Result<Arc<CredentialStore>, KeyringError> {
    Err(KeyringError::NoDefaultStore)
}

/// The platform's keystore.
struct Platform(Arc<CredentialStore>);

impl Platform {
    fn entry(&self, user: &str) -> Result<keyring_core::Entry, Error> {
        self.0.build(SERVICE, user, None).map_err(failed)
    }
}

impl SecretStore for Platform {
    fn get(&self, user: &str) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        match self.entry(user)?.get_secret() {
            Ok(record) => Ok(Some(Zeroizing::new(record))),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    fn set(&self, user: &str, record: &[u8]) -> Result<(), Error> {
        self.entry(user)?.set_secret(record).map_err(failed)
    }

    fn delete(&self, user: &str) -> Result<(), Error> {
        match self.entry(user)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(failed(error)),
        }
    }
}

/// A keystore that refuses: `KeyStoreUnavailable` when it cannot be reached
/// or refuses access, `StorageFailed` otherwise. The message carries the
/// kind of the failure, never a secret: an entry's bytes are left out.
fn failed(error: KeyringError) -> Error {
    match error {
        KeyringError::PlatformFailure(_)
        | KeyringError::NoStorageAccess(_)
        | KeyringError::NoDefaultStore => unavailable(error),
        KeyringError::BadEncoding(_) | KeyringError::BadDataFormat(..) => Error::new(
            ErrorCode::StorageFailed,
            "the keystore entry of the database cannot be read",
        ),
        error => Error::new(
            ErrorCode::StorageFailed,
            format!("the keystore failed: {error}"),
        ),
    }
}

/// `KeyStoreUnavailable`, whose message names `key_file` (RFC 0009, S26).
pub(crate) fn unavailable(error: KeyringError) -> Error {
    Error::new(
        ErrorCode::KeyStoreUnavailable,
        format!(
            "no platform keystore is reachable ({}); name a key file with key_file(path)",
            kind(&error)
        ),
    )
}

/// What failed, without anything the error holds.
fn kind(error: &KeyringError) -> &'static str {
    match error {
        KeyringError::PlatformFailure(_) => "the platform failed",
        KeyringError::NoStorageAccess(_) => "access was refused",
        KeyringError::NoDefaultStore => "this platform has none",
        _ => "it failed",
    }
}
