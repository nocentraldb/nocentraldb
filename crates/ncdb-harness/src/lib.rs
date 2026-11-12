//! The convergence harness of NoCentralDB (RFC 0011, "Checking a plugin:
//! ncdb-harness"): RFC 0004's plugin contract, C1 to C7, checked for a
//! field type, built in or plugin, natively and on wasm32.
//!
//! A plugin's author uses it as a dev-dependency. [`PluginState`] runs a
//! plugin's [`FieldType`](ncdb_plugin::FieldType) as the core does, on the
//! bytes the view would store, and also checks that `decode_state`
//! round-trips. The author writes a [`Generator`] of the type's operations,
//! usually through [`PluginState::write`], as an app writes, and broken
//! variants of the type that [`check`] must reject (C6). [`digest`] gives
//! the state bytes of a fixed run, which the native and the wasm32 tests
//! compare (C3).
//!
//! The core's built-in field types are checked by the same harness.
//!
//! The harness checks the contract, not whether the merge does what users
//! expect, how large the state grows, or whether the type is fast (RFC
//! 0004). It cannot prove a type converges: it tries many cases, and
//! prints the seed and a minimal counterexample of the first it breaks.

mod convergence;
mod plugin;

pub use convergence::{
    Author, Config, Generator, Independent, Member, Mergeable, Op, OpId, Outcome, Property,
    Violation, check, digest, independent,
};
pub use plugin::{PluginOp, PluginState, ReadBytes, operation_id, origin};
