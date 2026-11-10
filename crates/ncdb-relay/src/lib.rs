//! The NoCentralDB relay core, as RFC 0007 (`docs/rfc/0007-relay.md`)
//! defines it.
//!
//! All checks live here, the same for every deployment; a backend only
//! stores. The backend interface is [`store::RelayStore`], the one test
//! suite every backend passes is [`conformance`], and the first backend,
//! SQLite, is [`sqlite`].

pub mod conformance;
pub mod record;
pub mod sqlite;
pub mod store;
