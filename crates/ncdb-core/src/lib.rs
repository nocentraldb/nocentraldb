//! NoCentralDB core.
//!
//! This crate holds the change and the log as RFC 0002
//! (`docs/rfc/0002-change-and-log-format.md`) defines them: the canonical
//! CBOR encoding ([`cbor`]), the ids ([`ids`]), the change with its frame,
//! header, payload and operations ([`change`]), the hybrid logical clock
//! ([`clock`]) and an append-only in-memory log with a cursor ([`log`]).
//! Signatures and encryption are not here yet (RFC 0003, NOC-15): the
//! signature and the sealed payload are opaque byte strings.

pub mod cbor;
pub mod change;
pub mod clock;
pub mod ids;
pub mod log;
pub mod receive;
mod redact;

/// Returns the crate name; exists so the gates have real code to check.
pub fn name() -> &'static str {
    "ncdb-core"
}
