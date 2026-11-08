//! NoCentralDB core.
//!
//! This crate holds the change and the log as RFC 0002
//! (`docs/rfc/0002-change-and-log-format.md`) defines them: the canonical
//! CBOR encoding ([`cbor`]), the ids ([`ids`]), the change with its frame,
//! header, payload and operations ([`change`]), the hybrid logical clock
//! ([`clock`]) and an append-only in-memory log with a cursor ([`log`]).
//! Signatures and encryption are not here yet (RFC 0003, NOC-15): the
//! signature and the sealed payload are opaque byte strings.
//!
//! It also holds the field types of RFC 0004
//! (`docs/rfc/0004-merge-rules-basic-types.md`): what a field type defines
//! ([`field`]), basic values ([`basic`]), sets ([`set`]), links ([`link`])
//! and what links resolve against, the objects of a copy ([`object`]).

pub mod basic;
pub mod cbor;
pub mod change;
pub mod clock;
pub mod field;
pub mod ids;
pub mod link;
pub mod log;
pub mod object;
mod redact;
pub mod set;

/// Returns the crate name; exists so the gates have real code to check.
pub fn name() -> &'static str {
    "ncdb-core"
}
