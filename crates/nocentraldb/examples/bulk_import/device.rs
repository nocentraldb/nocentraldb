//! What both devices share: opening a database, waiting for the relay, and
//! the size of a database on disk.

use std::path::Path;
use std::time::{Duration, Instant};

use nocentraldb::{Connection, Db, OpenOptions};

/// The lock of the SDK tests' model, `tests/model/model.ts`, whose
/// `tasks` stand for the imported records.
pub const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// How long the measurement waits for the relay before it gives up.
pub const PATIENCE: Duration = Duration::from_secs(30 * 60);

/// Opens the database `name` in `dir`, its keys in a file next to it, as a
/// headless app keeps them; syncing with `relay`, if given.
pub async fn open(
    dir: &Path,
    name: &str,
    relay: Option<&str>,
    pairing: bool,
) -> Result<Db, String> {
    let mut options = OpenOptions::new(LOCK)
        .path(dir.join(name))
        .key_file(dir.join(format!("{name}.keys")));
    if let Some(url) = relay {
        options = options.relay(url);
    }
    if pairing {
        options = options.pairing();
    }
    Db::open(options)
        .await
        .map_err(|e| format!("open {name}: {e}"))
}

/// Waits until `done` holds, checking every 20 ms, or fails after
/// [`PATIENCE`].
pub async fn until(
    what: &str,
    mut done: impl FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let start = Instant::now();
    while !done()? {
        if start.elapsed() > PATIENCE {
            return Err(format!("{what}: not within {PATIENCE:?}"));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(())
}

/// Waits until `db` is connected and the relay holds every change it
/// wrote. Fails on an error of syncing, or once the relay refused a
/// change.
pub async fn synced(db: &Db) -> Result<(), String> {
    until("the relay holds every change", || {
        let status = db.sync().current();
        if let Some(error) = status.error {
            return Err(format!("syncing failed: {error}"));
        }
        if status.refused > 0 {
            return Err(format!("the relay refused {} changes", status.refused));
        }
        Ok(status.connection == Connection::Connected && status.unconfirmed == 0)
    })
    .await
}

/// The bytes of the database in the directory `dir`: its SQLite file, and
/// its write-ahead log, which SQLite folds back into the file and which
/// can grow while a device folds many changes.
pub fn size(dir: &Path) -> (u64, u64) {
    let len = |name: &str| std::fs::metadata(dir.join(name)).map_or(0, |m| m.len());
    (len("nocentraldb.sqlite"), len("nocentraldb.sqlite-wal"))
}
