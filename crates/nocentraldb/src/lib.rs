//! NoCentralDB's Rust SDK (RFC 0009, `docs/rfc/0009-sdk-api.md`): the calls
//! of the TypeScript SDK in snake case, fields read and written by name
//! with [`Value`] (decision S18).
//!
//! ```
//! # use nocentraldb::{Db, OpenOptions, Value};
//! # async fn example(lock: &str) -> Result<(), nocentraldb::Error> {
//! let db = Db::open(OpenOptions::new(lock)).await?;
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
//! Every call runs on the device and never waits for the network (RFC
//! 0006, "Reads never touch the network"). This SDK has no relay and no
//! sync yet; see [`Db`] for what else it leaves out.

mod db;
mod encode;
mod error;
mod id;
mod object;
mod space;
mod tx;
mod value;

pub use db::{Db, OpenOptions};
pub use error::{Error, ErrorCode};
pub use id::{ChangeId, ObjectId, SpaceId};
pub use object::Object;
pub use space::{Space, Spaces, Written};
pub use tx::Transaction;
pub use value::Value;
