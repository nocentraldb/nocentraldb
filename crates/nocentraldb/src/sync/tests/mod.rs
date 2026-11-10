//! The SDK syncing with the relay server on SQLite, run locally (RFC 0007,
//! "The protocol" and "Island mode"; RFC 0009, "Sync and status").
//!
//! These tests sit inside the crate because they open a database as a
//! simulated member with a known member key and account key: the relay
//! must let that account create spaces, and another simulated member must
//! add it to a space. The SDK keeps its keys to itself (RFC 0009), and has
//! no key file yet (NOC-108).

#[path = "../../../../ncdb-relay-server/tests/support/mod.rs"]
mod relay_support;

mod backoff;
mod fake;
mod pull;
mod push;
mod reconnect;
mod spaces;
mod support;
mod timeouts;
