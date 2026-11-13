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
//! On AWS, with the feature `aws`: `dynamo` is the `connections` table,
//! which [`contract`] holds to the same rules as the in-memory store;
//! `apigw` is the connection API; `s3` hands out presigned S3 URLs for
//! large changes; `settings` reads the function's environment. The binary,
//! which needs the feature, runs them over the AWS backend of
//! `ncdb-relay-aws`.
//!
//! Limits, as RFC 0007 has them: API Gateway refuses a message above
//! 128 KiB and closes every connection after 2 hours; the handler posts no
//! message above 128 KiB, sends a frame that does not fit inline as a
//! download URL, and lets a connection's record expire 2 hours after it
//! opened.
//!
//! In tests and in the local harness (`harness`, with the feature
//! `harness`), it runs on the SQLite backend and the in-memory
//! connections. Its infrastructure and deploy are `infra/aws` (NOC-272).

#[cfg(feature = "aws")]
pub mod apigw;
pub mod connections;
pub mod contract;
#[cfg(feature = "aws")]
pub mod dynamo;
pub mod event;
pub mod handler;
#[cfg(feature = "harness")]
pub mod harness;
pub mod log;
#[cfg(feature = "aws")]
pub mod s3;
#[cfg(feature = "aws")]
pub mod settings;

pub use connections::{ConnectionApi, Connections, MemoryConnections, Posted};
pub use event::{Body, Event, Route};
pub use handler::{Handler, Urls};
pub use log::Line;
