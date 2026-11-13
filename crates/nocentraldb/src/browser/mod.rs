//! The database in the browser (RFC 0006, "The browser"; NOC-175, as the
//! owner decided NOC-231): on the Origin Private File System, through the
//! core's `opfs-sahpool` VFS, in a dedicated Web Worker.
//!
//! [`place`] prepares the database named `name`: the file
//! `/<name>/nocentraldb.sqlite` in the pool, as a native database is
//! `nocentraldb.sqlite` in its directory, with two slots reserved for it
//! and its journal; its keys from IndexedDB under the same name, new ones
//! only for a database that did not exist (`keys::browser`); and how
//! durable the copy is. A name is 1 to 64 characters of `a`–`z`, `0`–`9`,
//! `-` and `_`. Where OPFS is missing, as in a private window of Firefox or
//! WebKit, the database runs in memory and the copy is temporary. An open
//! whose pool another tab holds fails with `StorageFailed`, until the owner
//! tab is built (NOC-176).

#[cfg(test)]
mod tests;

use js_sys::{Promise, Reflect};
use ncdb_core::view::browser::{InstallError, Storage as Vfs, exists, install, reserve};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::durability::Storage;
use crate::error::{Error, ErrorCode};
use crate::keys::{Keys, browser::load_or_create};

/// The file of a database, inside its name's directory, as natively.
const FILE: &str = "nocentraldb.sqlite";
/// The pool slots a database takes: its file and its rollback journal.
const SLOTS: usize = 2;
/// The longest name a database may have.
const LONGEST: usize = 64;

/// A database prepared at its name: where its view opens, its keys, and
/// how durable its copy is.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct Placed {
    /// The path of its file, in the VFS `install` made the default.
    pub(crate) path: String,
    /// The member's keys.
    pub(crate) keys: Keys,
    /// Where the copy is stored.
    pub(crate) storage: Storage,
    /// The database's name, under which IndexedDB keeps its keys.
    pub(crate) name: String,
}

/// Whether `name` may name a database: 1 to 64 characters of `a`–`z`,
/// `0`–`9`, `-` and `_`, since it becomes a path on OPFS and the name of
/// an IndexedDB database. Another is refused with `InvalidValue`.
pub(crate) fn valid_name(name: &str) -> Result<(), Error> {
    let allowed = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_';
    match !name.is_empty() && name.len() <= LONGEST && name.chars().all(allowed) {
        true => Ok(()),
        false => Err(Error::new(
            ErrorCode::InvalidValue,
            "a database name is 1 to 64 characters of a-z, 0-9, - and _",
        )),
    }
}

/// Prepares the database named `name` in this dedicated worker: installs
/// the VFS, reserves its slots, and takes its keys from IndexedDB.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) async fn place(name: &str, new: impl FnOnce() -> Keys) -> Result<Placed, Error> {
    valid_name(name)?;
    let vfs = install().await.map_err(refused)?;
    let path = format!("/{name}/{FILE}");
    let existed = exists(&path).await.map_err(refused)?;
    if vfs == Vfs::Opfs {
        reserve(SLOTS).await.map_err(refused)?;
    }
    let keys = load_or_create(name, existed, new).await?;
    let storage = match vfs {
        Vfs::Opfs if persisted().await => Storage::Persistent,
        Vfs::Opfs => Storage::BestEffort,
        Vfs::Memory => Storage::Temporary,
    };
    Ok(Placed {
        path,
        keys,
        storage,
        name: name.to_owned(),
    })
}

/// Whether the browser keeps this origin's storage persistent: the
/// worker's `navigator.storage.persisted()`. The page asks for it, since a
/// worker cannot (RFC 0006, "What the SDK reports"); `false` when the
/// browser does not say.
async fn persisted() -> bool {
    let call = || -> Option<Promise> {
        let navigator = Reflect::get(&js_sys::global(), &JsValue::from_str("navigator")).ok()?;
        let storage = Reflect::get(&navigator, &JsValue::from_str("storage")).ok()?;
        let persisted = Reflect::get(&storage, &JsValue::from_str("persisted")).ok()?;
        let promise = js_sys::Function::from(persisted).call0(&storage).ok()?;
        Some(Promise::from(promise))
    };
    match call() {
        Some(promise) => JsFuture::from(promise)
            .await
            .ok()
            .and_then(|answer| answer.as_bool())
            .unwrap_or(false),
        None => false,
    }
}

/// What a failure of the VFS means for the open: outside a dedicated
/// worker, `InvalidValue`; on OPFS, `StorageFailed`, most often because
/// another tab holds the pool.
fn refused(error: InstallError) -> Error {
    match error {
        InstallError::NotInWorker => Error::new(
            ErrorCode::InvalidValue,
            "a database at a name opens only in a dedicated Web Worker",
        ),
        error => Error::new(
            ErrorCode::StorageFailed,
            format!("the database cannot be opened: another tab holds it, or {error}"),
        ),
    }
}
