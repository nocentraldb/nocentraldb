//! NoCentralDB's Rust SDK (RFC 0009, `docs/rfc/0009-sdk-api.md`): the calls
//! of the TypeScript SDK in snake case, fields read and written by name
//! with [`Value`] (decision S18).
//!
//! ```
//! # use nocentraldb::{Db, OpenOptions, Value};
//! # async fn example(lock: &str) -> Result<(), nocentraldb::Error> {
//! let db = Db::open(OpenOptions::new(lock).in_memory()).await?;
//! let project = db.spaces().create().await?;
//! let written = project.write(|tx| {
//!     tx.create("tasks", [("title", Value::from("Ship the SDK"))])?;
//!     Ok(())
//! }).await?;
//! let task = project.get("tasks", written.ids[0]).await?;
//! # Ok(())
//! # }
//! ```
//!
//! A database lives in the directory [`OpenOptions::path`] names, with its
//! keys in the platform keystore or the file [`OpenOptions::key_file`]
//! names, and is found again when it is reopened; [`OpenOptions::in_memory`]
//! keeps both in memory, for tests (RFC 0009, S19).
//!
//! Every call runs on the device and never waits for the network (RFC
//! 0006, "Reads never touch the network"). A database opened with
//! [`OpenOptions::relay`] syncs with that relay in the background (RFC
//! 0007), and [`Db::sync`] reports how far; see [`Db`] for what the SDK
//! leaves out.

mod db;
mod encode;
mod error;
mod id;
mod keys;
mod next;
mod object;
mod query;
mod space;
mod subscribe;
mod sync;
mod tx;
mod value;

pub use db::{Db, OpenOptions};

#[cfg(test)]
mod agent_tests;
pub use error::{Error, ErrorCode};
pub use id::{ChangeId, ObjectId, SpaceId};
pub use object::Object;
pub use query::{Cursor, Page, Query};
pub use space::{Space, Spaces, Written};
pub use sync::{Connection, SyncReport, SyncStatus};
pub use tx::Transaction;
pub use value::Value;
