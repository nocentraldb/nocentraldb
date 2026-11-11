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
//! The `keys`, `person-keys` and `put-person-keys` requests of "Keys and the
//! person-key store" are [`Relay::keys`], [`Relay::person_keys`] and
//! [`Relay::put_person_keys`].
//!
//! After an append that can change a shred's stability, the relay checks
//! whether the shred is stable in its log (RFC 0008, "Effects that wait
//! until a change is stable"), but keeps the shredded key's record: the
//! deletion waits for NOC-224, since a stable shred can still be rejected
//! later by an admin key that was not yet revoked (NOC-225). While the
//! record stays stored, no new record of its id is accepted.
//!
//! **What this does not do yet.** No uploads of large changes; no restore
//! procedure beyond drawing a new epoch; no deletion of a stable shred's
//! person-key record, which waits for NOC-224.

pub mod auth;
pub mod conformance;
mod error;
mod keys;
pub mod record;
mod relay;
mod shred;
pub mod sqlite;
pub mod store;

pub use error::ErrorCode;
pub use keys::Keys;
pub use relay::{Config, MAX_FRAME, Pulled, Push, Pushed, Relay};
