//! The view in a browser (RFC 0006, "The browser"): SQLite on the Origin
//! Private File System, through the `opfs-sahpool` VFS of
//! `sqlite-wasm-vfs`, in a dedicated Web Worker.
//!
//! The one asynchronous step is [`install`], once per worker: it makes
//! `opfs-sahpool` SQLite's default VFS. After it, [`View::open`] opens the
//! database at a path as it does natively, and every SQLite call is
//! synchronous inside the worker (RFC 0006, "The same fold code on every
//! target"). Where OPFS is missing, as in a private window of Firefox or
//! WebKit, SQLite's in-memory VFS stays the default, and the copy is
//! temporary (RFC 0006, decision 12).
//!
//! [`View::open`]: super::View::open

use std::fmt;

use sqlite_wasm_rs::WasmOsCallback;
use sqlite_wasm_rs::vfs::VfsFilesManager;
use sqlite_wasm_vfs::sahpool::{self, OpfsSAHError, OpfsSAHPoolCfg};
use wasm_bindgen::JsCast;
use web_sys::DedicatedWorkerGlobalScope;

/// Where the database of a view opened after [`install`] lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// On OPFS: the browser keeps it across page loads, until it evicts the
    /// origin's data (RFC 0006, "Durability of a browser copy").
    Opfs,
    /// In the worker's memory: it is gone when the worker ends. OPFS is
    /// missing, as in a private window of Firefox or WebKit.
    Memory,
}

/// Why [`install`] failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum InstallError {
    /// The code runs outside a dedicated Web Worker, where OPFS
    /// synchronous access handles never exist (RFC 0006, "The browser").
    NotInWorker,
    /// OPFS exists, and the pool on it could not be opened: for example,
    /// another tab's worker holds it (RFC 0006, decision 4).
    Opfs(OpfsSAHError),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::NotInWorker => {
                f.write_str("the view runs only in a dedicated Web Worker")
            }
            InstallError::Opfs(error) => write!(f, "OPFS could not be opened: {error}"),
        }
    }
}

impl std::error::Error for InstallError {}

/// Makes `opfs-sahpool` SQLite's default VFS in this worker, so that a
/// view opened afterwards lives on OPFS; a second call reuses the pool.
/// Where OPFS is missing, it leaves the in-memory VFS as the default and
/// returns [`Storage::Memory`] (RFC 0006, decision 12).
pub async fn install() -> Result<Storage, InstallError> {
    if !js_sys::global().is_instance_of::<DedicatedWorkerGlobalScope>() {
        return Err(InstallError::NotInWorker);
    }
    match sahpool::install::<WasmOsCallback>(&OpfsSAHPoolCfg::default(), true).await {
        Ok(_) => Ok(Storage::Opfs),
        Err(error) if opfs_is_missing(&error) => Ok(Storage::Memory),
        Err(error) => Err(InstallError::Opfs(error)),
    }
}

/// Whether a database file exists at `path` on OPFS, in the pool
/// [`install`] makes the default VFS, which it installs or reuses; `false`
/// where OPFS is missing, since a copy in memory is gone with its worker.
/// It fails as `install` does: outside a dedicated Web Worker, or when the
/// pool cannot be opened.
pub async fn exists(path: &str) -> Result<bool, InstallError> {
    if !js_sys::global().is_instance_of::<DedicatedWorkerGlobalScope>() {
        return Err(InstallError::NotInWorker);
    }
    match sahpool::install::<WasmOsCallback>(&OpfsSAHPoolCfg::default(), true).await {
        Ok(pool) => pool.contains(path).map_err(InstallError::Opfs),
        Err(error) if opfs_is_missing(&error) => Ok(false),
        Err(error) => Err(InstallError::Opfs(error)),
    }
}

/// Whether `error` says the browser has no OPFS here: the API is missing,
/// or the browser refuses the OPFS root, as Firefox and WebKit do in a
/// private window (NOC-33's findings, "Private window").
fn opfs_is_missing(error: &OpfsSAHError) -> bool {
    match error {
        OpfsSAHError::NotSupported => true,
        OpfsSAHError::Opfs { operation, .. } => *operation == "get OPFS root",
        _ => false,
    }
}
