//! Reading `model.lock.json`, and the description of a schema that
//! `ncdb model build` gets from the TypeScript module.

use std::fmt;

use super::ModelLock;

/// Why a lock (or a schema's description) could not be read: the JSON
/// path of the offending value, and what is wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockError {
    /// Where, such as `collections.tasks.fields.title.since`; empty for the
    /// whole document.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for LockError {}

impl ModelLock {
    /// Reads a lock from its JSON text.
    pub fn from_json(_text: &str) -> Result<ModelLock, LockError> {
        Err(LockError {
            path: String::new(),
            message: "not implemented".into(),
        })
    }
}
