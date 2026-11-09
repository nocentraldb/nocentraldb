//! The additive-change rule (RFC 0005, "The additive-change rule").

use super::{ModelLock, Refusal};

/// What the lock breaks on its own.
pub fn validate(_lock: &ModelLock) -> Vec<Refusal> {
    Vec::new()
}

/// What `next` breaks as a successor of `previous`.
pub fn successor(_previous: &ModelLock, _next: &ModelLock) -> Vec<Refusal> {
    Vec::new()
}
