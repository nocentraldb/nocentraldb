//! NoCentralDB core.
//!
//! This crate holds the change and the log as RFC 0002
//! (`docs/rfc/0002-change-and-log-format.md`) defines them: the canonical
//! CBOR encoding ([`cbor`]), the ids ([`ids`]), the change with its frame,
//! header, payload and operations ([`change`]), the hybrid logical clock
//! ([`clock`]) and an append-only in-memory log with a cursor ([`log`]).
//! It also holds the member and space keys ([`keys`]) and signing and
//! sealing a change as RFC 0003 (`docs/rfc/0003-keys-and-cryptography.md`)
//! defines it ([`seal`]).

pub mod cbor;
pub mod change;
pub mod clock;
pub mod ids;
pub mod keys;
pub mod log;
mod redact;
pub mod seal;

/// Returns the crate name; exists so the gates have real code to check.
pub fn name() -> &'static str {
    "ncdb-core"
}
