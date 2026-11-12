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
//! RFC 0007's messages as bytes are [`wire`], and one `entries` message of
//! a pull or a subscription is [`page`]: every deployment speaks them, the
//! WebSocket server of the crate `ncdb-relay-server` and the Lambda
//! function on AWS alike.
//!
//! The `keys`, `person-keys` and `put-person-keys` requests of "Keys and the
//! person-key store" are [`Relay::keys`], [`Relay::person_keys`] and
//! [`Relay::put_person_keys`].
//!
//! When the relay accepts a shred, it deletes the shredded key's record at
//! once (RFC 0008, "A shred is final when accepted"): no later change can
//! reject an accepted shred. No new record of a shredded id is accepted.
//!
//! A frame too large for one message goes up through an upload and comes
//! down through a download token (RFC 0007, "Large changes"):
//! [`Relay::begin_upload`], [`Relay::fill_upload`], [`Relay::push_frames`],
//! [`Relay::begin_download`] and [`Relay::download`].
//!
//! **What this does not do yet.** No restore procedure beyond drawing a
//! new epoch.

pub mod auth;
pub mod conformance;
mod error;
mod keys;
mod large;
pub mod page;
pub mod record;
mod relay;
mod shred;
pub mod sqlite;
pub mod store;
pub mod wire;

pub use error::ErrorCode;
pub use keys::Keys;
pub use large::{DOWNLOAD_URL_VALID_FOR, Filled, PushFrame, UPLOAD_URL_VALID_FOR};
pub use relay::{Config, FINALIZE_AFTER, MAX_FRAME, PAGE_BYTES, Pulled, Push, Pushed, Relay};
