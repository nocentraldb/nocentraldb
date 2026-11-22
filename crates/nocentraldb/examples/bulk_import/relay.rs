//! The local relay: the `ncdb-relay` binary on a SQLite database of its
//! own, started as the Rust guide's step 8 starts it, and what it received,
//! read from that database after the import.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// A running `ncdb-relay`, stopped when dropped.
pub struct Relay {
    child: Child,
    /// The URL the devices sync with.
    pub url: String,
    /// The relay's SQLite database.
    pub database: PathBuf,
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Relay {
    /// Starts `binary` on a database in `dir`, on a free loopback port,
    /// letting the account `invitation` names create spaces.
    pub fn start(binary: &Path, dir: &Path, invitation: &str) -> Result<Relay, String> {
        let database = dir.join("relay.sqlite");
        let mut child = Command::new(binary)
            .arg("--database")
            .arg(&database)
            .args(["--listen", "127.0.0.1:0", "--allow-invitation", invitation])
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", binary.display()))?;
        let stdout = child.stdout.take().ok_or("no output")?;
        let mut line = String::new();
        BufReader::new(stdout)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        let url = line
            .trim()
            .strip_prefix("ncdb-relay listening on ")
            .ok_or_else(|| format!("unexpected output of ncdb-relay: {line}"))?
            .to_owned();
        Ok(Relay {
            child,
            url,
            database,
        })
    }

    /// The relay's resident memory at its peak, in bytes, where the
    /// platform reports it.
    pub fn peak_memory(&self) -> Option<u64> {
        crate::memory::peak(&format!("/proc/{}/status", self.child.id()))
    }

    /// What the relay holds: its log's changes and their frames' bytes,
    /// over every space, and the size of its database files.
    pub fn received(&self) -> Result<Received, String> {
        let db = rusqlite::Connection::open_with_flags(
            &self.database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(|e| e.to_string())?;
        let (changes, bytes, largest): (i64, i64, i64) = db
            .query_row(
                "SELECT count(*), coalesce(sum(length(frame)), 0), \
                 coalesce(max(length(frame)), 0) FROM log",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| e.to_string())?;
        // Frames above the inline limit went up through an upload URL and
        // come down through a download link (RFC 0007, "Large changes").
        let large: i64 = db
            .query_row(
                "SELECT count(*) FROM log WHERE length(frame) > ?1",
                [MAX_INLINE as i64],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut files = 0;
        for suffix in ["", "-wal"] {
            let mut path = self.database.as_os_str().to_owned();
            path.push(suffix);
            files += std::fs::metadata(PathBuf::from(path)).map_or(0, |m| m.len());
        }
        let count = |n: i64| u64::try_from(n).unwrap_or(0);
        Ok(Received {
            changes: count(changes),
            bytes: count(bytes),
            largest: count(largest),
            large: count(large),
            files,
        })
    }
}

/// The largest frame a relay message carries inline, 22 KiB (RFC 0007;
/// NOC-338): a larger change goes up through an upload.
pub const MAX_INLINE: u64 = 22 * 1024;

/// What the relay received.
#[derive(Debug)]
pub struct Received {
    /// Changes in its logs, the space's own control changes included.
    pub changes: u64,
    /// Their frames' bytes.
    pub bytes: u64,
    /// The largest frame.
    pub largest: u64,
    /// Frames above [`MAX_INLINE`], which went through an upload.
    pub large: u64,
    /// The database's files on disk.
    pub files: u64,
}
