//! The check on the device (RFC 0005, "Two checks: at build and on the
//! device"): the view keeps, in `meta`, the lock its database was
//! last opened with, as canonical JSON, and runs the additive check
//! between that lock and the one it is opened with now.
//!
//! - A lock that is an additive successor of the stored one opens the
//!   database and takes its place.
//! - A lock of which the stored one is an additive successor, an app
//!   rolled back to an older version, opens the database as an old client
//!   (RFC 0005, "Two checks"). The stored lock stays: a later lock must
//!   still be a successor of the newest one the device has opened, so that
//!   a lock that forks from the older one cannot fold data written under
//!   the newer one by other rules.
//! - Any other lock is refused with every refused change, a lock of
//!   another `model.id` among them, and nothing is written.

use rusqlite::Connection;

use super::Error;
use super::schema::meta;
use crate::model_lock::{ModelLock, check::successor};

/// The key of the stored lock in `meta`.
const KEY: &str = "lock";

/// Checks `lock` against the lock the database was last opened with, and
/// stores it when it is that lock's successor. `current` says whether the
/// view's tables were folded by the model of `lock`.
///
/// A database written before the view kept its lock (NOC-108) has none to
/// check against: it opens only when its tables were folded by the model
/// of `lock`, and then keeps `lock`.
pub(super) fn check(conn: &Connection, lock: &ModelLock, current: bool) -> Result<(), Error> {
    let Some(text) = meta(conn, KEY)? else {
        return match current {
            true => store(conn, lock),
            false => Err(Error::Model),
        };
    };
    let stored = ModelLock::from_json(&text).map_err(|_| Error::Corrupt("meta lock"))?;
    if stored == *lock {
        return Ok(());
    }
    let refused = successor(&stored, lock);
    if refused.is_empty() {
        return store(conn, lock);
    }
    if successor(lock, &stored).is_empty() {
        return Ok(());
    }
    Err(Error::Refused(refused))
}

/// Keeps `lock` as the lock the database was last opened with.
pub(super) fn store(conn: &Connection, lock: &ModelLock) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT DO UPDATE SET value = excluded.value",
        [KEY, &lock.to_canonical_json()],
    )?;
    Ok(())
}
