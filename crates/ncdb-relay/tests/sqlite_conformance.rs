//! The SQLite backend passes the one backend test suite of RFC 0007.

use std::path::PathBuf;

use ncdb_relay::conformance::Backend;
use ncdb_relay::sqlite::SqliteStore;

/// A relay database in its own temporary directory, removed on drop.
struct Sqlite {
    dir: PathBuf,
}

impl Backend for Sqlite {
    type Store = SqliteStore;

    fn connect(&self) -> SqliteStore {
        SqliteStore::open(&self.dir.join("relay.sqlite")).expect("the database opens")
    }
}

impl Drop for Sqlite {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fresh() -> Sqlite {
    let mut name = [0u8; 16];
    getrandom::fill(&mut name).expect("a secure generator");
    let name: String = name.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("ncdb-relay-{name}"));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    Sqlite { dir }
}

ncdb_relay::conformance_suite!(fresh);
ncdb_relay::token_urls_conformance_suite!(fresh);
