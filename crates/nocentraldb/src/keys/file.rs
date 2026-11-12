//! The keys in a key file the app names, for a headless agent without a
//! platform keystore (RFC 0003, "The member key"; RFC 0009, "The Rust
//! SDK"). Only the
//! file's permissions protect it: on Unix the SDK creates it readable and
//! writable by its user alone.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use zeroize::Zeroizing;

use super::{Keys, io};
use crate::error::{Error, ErrorCode};

/// The keys in the key file at `path`. When the file does not exist and
/// the database did not exist either, new keys are drawn and written to
/// a new file. An existing database whose key file is gone fails with
/// `StorageFailed`: new keys would hold none of its spaces' keys.
pub(crate) fn from_file(path: &Path, database_existed: bool) -> Result<Keys, Error> {
    match read(path)? {
        Some(keys) => Ok(keys),
        None if database_existed => Err(Error::new(
            ErrorCode::StorageFailed,
            "the database exists, but its key file does not; new keys would open none of its spaces",
        )),
        None => create(path),
    }
}

/// The keys in the file, or `None` when there is no file.
fn read(path: &Path) -> Result<Option<Keys>, Error> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io("the key file cannot be read", &error)),
    };
    let mut record = Zeroizing::new(Vec::new());
    file.read_to_end(&mut record)
        .map_err(|error| io("the key file cannot be read", &error))?;
    Keys::decode(&record).map(Some).ok_or_else(|| {
        Error::new(
            ErrorCode::StorageFailed,
            "the key file holds no keys of this SDK's format",
        )
    })
}

/// Draws new keys and writes them to a new file at `path`, which must not
/// exist yet.
fn create(path: &Path) -> Result<Keys, Error> {
    let keys = Keys::generate();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options.open(path).and_then(|mut file| {
        file.write_all(&keys.encode())?;
        file.sync_all()
    });
    match written {
        Ok(()) => Ok(keys),
        Err(error) => {
            // A file that holds part of a record would refuse every later
            // open; one that was never created is not removed.
            if error.kind() != ErrorKind::AlreadyExists {
                let _ = std::fs::remove_file(path);
            }
            Err(io("the key file cannot be written", &error))
        }
    }
}
