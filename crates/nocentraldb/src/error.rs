//! The SDK's one error, with a code and the ids it concerns, never content,
//! a key or a personal field; `UnknownAccount` also carries the account's
//! public fingerprint (RFC 0009, "Errors"; RFC 0003, "Keys never
//! reach logs, errors or telemetry").

use std::fmt;

use crate::members::Fingerprint;

/// What the SDK refused, or why a call failed. Its message names the call
/// and the ids it concerns, never a value.
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    code: ErrorCode,
    message: String,
    fingerprint: Option<Fingerprint>,
}

/// The codes of [`Error`], RFC 0009's codes in Rust case. Only the codes
/// of the calls this SDK has are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// The lock cannot be read, or breaks the rules RFC 0005 gives a lock,
    /// or is refused against the lock the database was last opened with:
    /// another `model.id`, or a change that is not additive (RFC 0005,
    /// D11). The message names each refused change.
    ModelRefused,
    /// The disk or the quota is full: freeing space may help.
    StorageFull,
    /// The database could not store or read what a call asked for another
    /// reason than a full disk, such as SQLite failing or a corrupt file.
    StorageFailed,
    /// The copy holds no such object, or the model no such collection,
    /// field or index.
    NotFound,
    /// A value does not fit the field's kind.
    InvalidValue,
    /// The space's genesis names another `model.id` than the lock's; the
    /// message names both.
    SpaceOfAnotherModel,
    /// The cursor was made under another model, index or equality prefix.
    CursorOfAnotherModel,
    /// No platform keystore is reachable and the app named no key file;
    /// the message names `key_file` (RFC 0009, S26).
    KeyStoreUnavailable,
    /// The invitation's account is new to this database, and no
    /// fingerprint of it was passed to `members().add`. The error's
    /// [`fingerprint`](Error::fingerprint) is the account's.
    UnknownAccount,
    /// The member's role does not allow the call.
    NotAllowed,
    /// The member is no longer a member of the space.
    NotMember,
    /// The invitation's certificate does not hold: its account did not
    /// sign it, it certifies another member than the invitation's keys, or
    /// it is a sub-agent's. It can mean a forged invitation, which a person
    /// may need to see.
    InvalidCertificate,
    /// The invitation's member is a member of the space already.
    AlreadyMember,
}

impl Error {
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Error {
        Error {
            code,
            message: message.into(),
            fingerprint: None,
        }
    }

    /// The error with the fingerprint of the account it concerns.
    pub(crate) fn with_fingerprint(self, fingerprint: Fingerprint) -> Error {
        Error {
            fingerprint: Some(fingerprint),
            ..self
        }
    }

    /// The error's code.
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    /// The fingerprint of the account the error concerns: set on
    /// `UnknownAccount`, for a person to compare before the app passes it
    /// to `members().add` (RFC 0009, "Invitations and members").
    pub fn fingerprint(&self) -> Option<&Fingerprint> {
        self.fingerprint.as_ref()
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error({:?}: {})", self.code, self.message)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

/// A failure of the view, which only the database itself can cause:
/// `StorageFull` when the disk or the quota is full, `StorageFailed`
/// otherwise (NOC-95, 4 B).
pub(crate) fn storage(error: ncdb_core::view::Error) -> Error {
    let code = match error.is_full() {
        true => ErrorCode::StorageFull,
        false => ErrorCode::StorageFailed,
    };
    Error::new(code, format!("the local view failed: {error}"))
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// Only a full disk tells the app to free space (NOC-95, 4 B); the
    /// core's own test holds which SQLite errors are full.
    #[test]
    fn a_failure_of_the_view_that_is_not_a_full_disk_is_storage_failed() {
        let error = storage(ncdb_core::view::Error::Corrupt("log"));

        assert_eq!(error.code(), ErrorCode::StorageFailed);
    }
}
