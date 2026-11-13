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

mod account_space;
mod added;
mod auth;
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
mod lost_answer;
mod lost_device;
mod members;
mod merge;
mod newest_model;
mod pairing;
mod pause;
mod person_keys;
mod personal;
mod provisional;
mod pull;
mod push;
mod push_again;
mod reconnect;
mod refusals;
mod remove_member;
mod removed;
mod renewed_key;
mod reopen;
mod rotation;
mod space_status;
mod space_status_sync;
mod spaces;
mod spaces_subscribe;
pub(crate) mod support;
mod timeouts;
mod two_devices;
mod wss;
