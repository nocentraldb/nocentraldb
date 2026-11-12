//! The SDK syncing with the relay server on SQLite, run locally (RFC 0007,
//! "The protocol" and "Island mode"; RFC 0009, "Sync and status").
//!
//! These tests sit inside the crate because they open a database as a
//! simulated member with a known member key and account key: the relay
//! must let that account create spaces, and another simulated member must
//! add it to a space. The SDK keeps its keys to itself (RFC 0009); only
//! `reopen` writes them into a key file of the SDK's format first.

#[path = "../../../../ncdb-relay-server/tests/support/mod.rs"]
mod relay_support;

mod added;
mod backoff;
mod codes;
mod confirmed;
mod counts;
mod erasures;
mod fake;
mod follow;
mod large;
mod list;
mod live;
mod members;
mod merge;
mod newest_model;
mod pause;
mod person_keys;
mod pull;
mod push;
mod push_again;
mod reconnect;
mod refusals;
mod removed;
mod reopen;
mod rotation;
mod space_status;
mod space_status_sync;
mod spaces;
pub(crate) mod support;
mod timeouts;
mod two_devices;
mod wss;
