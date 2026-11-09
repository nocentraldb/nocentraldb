//! Writing `model.lock.json` in its one canonical form, and the model hash.

use serde_json::Value;

use super::ModelLock;

impl ModelLock {
    /// The lock as a JSON value.
    pub fn to_json(&self) -> Value {
        Value::Null
    }

    /// The lock in its canonical form, as `ncdb model build` writes it.
    pub fn to_canonical_json(&self) -> String {
        String::new()
    }

    /// The model hash of this lock's content.
    pub fn compute_hash(&self) -> String {
        String::new()
    }
}

/// The canonical JSON of RFC 8785.
pub fn canonical(_value: &Value) -> String {
    String::new()
}
