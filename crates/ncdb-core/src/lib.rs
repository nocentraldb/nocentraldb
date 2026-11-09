//! NoCentralDB core.
//!
//! This crate holds the change and the log as RFC 0002
//! (`docs/rfc/0002-change-and-log-format.md`) defines them: the canonical
//! CBOR encoding ([`cbor`]), the ids ([`ids`]), the change with its frame,
//! header, payload and operations ([`change`]), the hybrid logical clock
//! ([`clock`]) and an append-only in-memory log with a cursor ([`log`]).
//! It also holds the member and space keys ([`keys`]) and signing and
//! sealing a change as RFC 0003 (`docs/rfc/0003-keys-and-cryptography.md`)
//! defines it ([`seal`]). A member takes each change it receives through the
//! checks of RFC 0002 Amendment 1 and keeps it folded, parked or rejected
//! ([`receive`]); the signature, membership and decryption checks plug in
//! through [`receive::Checks`], and [`checks`] implements them with the real
//! signature and decryption.
//!
//! It also holds the field types of RFC 0004
//! (`docs/rfc/0004-merge-rules-basic-types.md`): what a field type defines
//! ([`field`]), basic values ([`basic`]), sets ([`set`]), links ([`link`])
//! and what links resolve against, the objects of a copy ([`object`]).
//! How their operations, and the creates and deletes of objects, travel in
//! a change, RFC 0004 Amendment 1, is [`field_op`].

pub mod basic;
pub mod cbor;
pub mod change;
pub mod checks;
pub mod clock;
pub mod field;
pub mod field_op;
pub mod ids;
pub mod keys;
pub mod link;
pub mod log;
pub mod model_lock;
pub mod object;
pub mod receive;
mod redact;
pub mod seal;
pub mod set;

/// Returns the crate name; exists so the gates have real code to check.
pub fn name() -> &'static str {
    "ncdb-core"
}
