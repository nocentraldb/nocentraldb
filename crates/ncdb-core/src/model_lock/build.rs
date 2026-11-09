//! From the description of a schema and the previous lock to the next lock,
//! as `ncdb model build` does.

use super::{LockError, ModelId, ModelLock, Refusal};

/// The description of a schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft;

impl Draft {
    /// Reads a schema's description from its JSON text.
    pub fn from_json(_text: &str) -> Result<Draft, LockError> {
        Err(LockError {
            path: String::new(),
            message: "not implemented".into(),
        })
    }
}

/// A lock that passed the additive-change rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The next lock.
    pub lock: ModelLock,
}

/// Builds the next lock from `draft` and the `previous` lock.
pub fn build(
    _draft: &Draft,
    _previous: Option<&ModelLock>,
    _new_id: impl FnOnce() -> ModelId,
) -> Result<Built, Vec<Refusal>> {
    Err(Vec::new())
}
