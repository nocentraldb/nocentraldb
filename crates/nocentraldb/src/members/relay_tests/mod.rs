//! `members()` against the relay server on SQLite, run locally (RFC 0009,
//! "Invitations and members"; RFC 0008, "Membership entry" and "Key chain
//! entry"): what an admin writes reaches the relay, which accepts it, and
//! the other members, which fold it.
//!
//! They share the sync tests' support, included by path, so that this
//! module needs nothing of `crate::sync` beyond what the SDK exposes.

#[path = "../../../../ncdb-relay-server/tests/support/mod.rs"]
#[allow(dead_code)]
mod relay_support;
#[path = "../../sync/tests/support.rs"]
#[allow(dead_code, unused_imports)]
mod support;

mod readd;
