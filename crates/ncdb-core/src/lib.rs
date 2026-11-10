//! NoCentralDB core.
//!
//! This crate holds the change and the log as RFC 0002
//! (`docs/rfc/0002-change-and-log-format.md`) defines them: the canonical
//! CBOR encoding ([`cbor`]), the ids ([`ids`]), the change with its frame,
//! header, payload and operations ([`change`]), the hybrid logical clock
//! ([`clock`]) and an append-only in-memory log with a cursor ([`log`]),
//! which keeps what a member's next change takes from it ([`tips`]).
//! It also holds the member and space keys ([`keys`]) and signing and
//! sealing a change as RFC 0003 (`docs/rfc/0003-keys-and-cryptography.md`)
//! defines it ([`seal`]), and the key envelopes that wrap a space key to one
//! member with HPKE ([`envelope`]). A member takes each change it receives through the
//! checks of RFC 0002 ("What a member does with a change it receives")
//! and keeps it folded, parked or rejected
//! ([`receive`]), and un-folds a folded change that a later control change
//! rejects; the signature, membership and decryption checks plug in
//! through [`receive::Checks`], and [`checks`] implements them with the real
//! signature and decryption.
//!
//! It also holds the field types of RFC 0004
//! (`docs/rfc/0004-merge-rules-basic-types.md`): what a field type defines
//! ([`field`]), basic values ([`basic`]), sets ([`set`]), links ([`link`])
//! and what links resolve against, the objects of a copy ([`object`]).
//! A reference into another space, RFC 0004's "References", is [`reference`].
//! How their operations, and the creates and deletes of objects, travel in
//! a change, RFC 0004's "How field operations travel in a change", is
//! [`field_op`].
//!
//! The genesis of a space, its membership entries and the membership state
//! that members and the relay fold from them, RFC 0008
//! (`docs/rfc/0008-membership.md`), are [`membership`].
//!
//! A member keeps its log and the view folded from it in SQLite, natively
//! and on wasm32, as RFC 0006 (`docs/rfc/0006-local-view.md`) defines it
//! ([`view`]).

pub mod basic;
pub mod cbor;
pub mod change;
pub mod checks;
pub mod clock;
pub mod envelope;
pub mod field;
pub mod field_op;
pub mod ids;
pub mod keys;
pub mod link;
pub mod log;
pub mod membership;
pub mod model_lock;
pub mod object;
pub mod receive;
mod redact;
pub mod reference;
pub mod seal;
pub mod set;
pub mod tips;
mod unfold;
pub mod view;

/// Returns the crate name; exists so the gates have real code to check.
pub fn name() -> &'static str {
    "ncdb-core"
}
