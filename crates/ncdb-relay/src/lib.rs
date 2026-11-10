//! The NoCentralDB relay core, as RFC 0007 (`docs/rfc/0007-relay.md`)
//! defines it.
//!
//! All checks live here, the same for every deployment; a backend only
//! stores. [`Relay`] runs "What the relay checks" on every pushed frame and
//! on every read, with RFC 0008's `resolve` from `ncdb-core` for
//! membership, and answers with the codes of "The errors" ([`ErrorCode`]).
//! [`auth`] verifies the signed challenge that opens a connection. The
//! backend interface is [`store::RelayStore`], the membership table's rows
//! are [`record`], the one test suite every backend passes is
//! [`conformance`], and the first backend, SQLite, is [`sqlite`].
//!
//! The network server, with RFC 0007's messages and subscriptions, is the
//! crate `ncdb-relay-server`.
//!
//! **What this does not do yet.** No `keys`, `person-keys` or
//! `put-person-keys` requests; no uploads of large changes; no restore
//! procedure beyond drawing a new epoch. The
//! relay refuses shred entries, whose second-account sign-off it cannot
//! check yet, as `unsupported_suite`.

pub mod auth;
pub mod conformance;
mod error;
pub mod record;
mod relay;
pub mod sqlite;
pub mod store;

pub use error::ErrorCode;
pub use relay::{Config, MAX_FRAME, Pulled, Pushed, Relay};
