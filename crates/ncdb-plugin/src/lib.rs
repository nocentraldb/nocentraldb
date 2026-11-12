//! What a NoCentralDB plugin field type implements and depends on (RFC
//! 0011, `docs/rfc/0011-plugin-field-types.md`).
//!
//! A plugin is a Rust crate of one namespace. It implements [`FieldType`]
//! for each of its types and exports a function `plugin()` that returns a
//! [`Plugin`], which the app hands to the SDK when it opens the database
//! (`OpenOptions::plugin`). The core then folds the fields of those types
//! through the plugin's code, natively and on wasm32 alike.
//!
//! ```ignore
//! pub fn plugin() -> ncdb_plugin::Plugin {
//!     ncdb_plugin::Plugin::new("acme")
//!         .field_type("counter", 1, CounterV1)
//!         .field_type("counter", 2, CounterV2)
//! }
//! ```
//!
//! This crate holds what a plugin needs and nothing else: the trait, the
//! [`Data`] a plugin reads and writes, the ids and stamp of an operation
//! ([`Origin`], [`OperationId`], [`ChangeId`], [`MemberId`], [`Hlc`]), the
//! three reasons a plugin may park an operation ([`ParkReason`]), and
//! [`Plugin`]. The view, SQLite, the keys and the bytes of a change stay
//! inside `ncdb-core`. The core also takes its canonical CBOR from here, so
//! that a plugin's `Data` and the core encode alike; that module is the
//! core's, and hidden.
//!
//! The core cannot prove that a plugin converges: it runs what the app
//! registers. A plugin's author checks it with `ncdb-harness` in the
//! plugin's own CI (RFC 0011, "Checking a plugin: ncdb-harness").

#[doc(hidden)]
pub mod cbor;
mod clock;
mod data;
mod field;
mod ids;
mod plugin;
mod redact;

pub use clock::Hlc;
pub use data::{Data, DataError};
pub use field::{ConfigError, FieldType, InvalidIntent, Origin, Outcome, ParkReason, Written};
pub use ids::{ChangeId, MemberId, OperationId};
pub use plugin::Plugin;
#[doc(hidden)]
pub use plugin::{ApplyError, ErasedField, ErasedType, WriteError};
