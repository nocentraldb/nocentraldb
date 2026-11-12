//! The NoCentralDB relay as one AWS Lambda function behind an API Gateway
//! WebSocket API (RFC 0007, "The AWS backend"): `$connect`, `$disconnect`
//! and `$default` invoke it, it runs the relay core of `ncdb-relay` on
//! each message, and it answers, delivers live entries to subscribers and
//! tells added members through API Gateway's connection API.
//!
//! [`event`] reads API Gateway's events, [`handler`] handles one,
//! [`connections`] holds what the relay knows of each open connection
//! between invocations and the connection API it posts through, and
//! [`log`] is the one log line of each invocation: ids, sizes, codes and
//! times, no content and no keys.
//!
//! Limits, as RFC 0007 has them: API Gateway refuses a message above
//! 128 KiB and closes every connection after 2 hours; the handler posts no
//! message above 128 KiB, sends a frame that does not fit inline as a
//! download URL, and lets a connection's record expire 2 hours after it
//! opened.
//!
//! **What this does not do yet.** It runs on the SQLite backend and the
//! in-memory connections, in tests and in the local harness
//! (`harness`, with the feature `harness`): the DynamoDB and S3 backend
//! (NOC-270), the `connections` table, and the client of the real
//! connection API (NOC-272) follow.

pub mod connections;
pub mod event;
pub mod handler;
#[cfg(feature = "harness")]
pub mod harness;
pub mod log;

pub use connections::{ConnectionApi, Connections, MemoryConnections, Posted};
pub use event::{Body, Event, Route};
pub use handler::{Handler, Urls};
pub use log::Line;
