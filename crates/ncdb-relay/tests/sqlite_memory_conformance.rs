//! The SQLite backend in memory passes the one backend test suite of
//! RFC 0007, as on a file: a clone of the store is another handle on the
//! same database, as another relay process would open a file.

use ncdb_relay::conformance::Backend;
use ncdb_relay::sqlite::SqliteStore;

/// A relay database in memory, gone with its last handle.
struct Memory(SqliteStore);

impl Backend for Memory {
    type Store = SqliteStore;

    fn connect(&self) -> SqliteStore {
        self.0.clone()
    }
}

fn fresh() -> Memory {
    Memory(SqliteStore::in_memory().expect("the database opens"))
}

ncdb_relay::conformance_suite!(fresh);
ncdb_relay::token_urls_conformance_suite!(fresh);
