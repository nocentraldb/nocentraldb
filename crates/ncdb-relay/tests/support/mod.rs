//! Helpers of the relay's tests. The simulated space, whose members write
//! real, signed changes, is the core's own test helper, so the relay is
//! tested with the changes the core's tests use.
#![allow(dead_code)]

#[path = "../../../ncdb-core/tests/support/space.rs"]
pub mod space;

pub mod counting;
pub mod interleave;

use std::collections::BTreeSet;
use std::path::PathBuf;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Pushed, Relay};

use space::{Who, account, id};

/// A temporary directory, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        let mut name = [0u8; 16];
        getrandom::fill(&mut name).expect("a secure generator");
        let name: String = name.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ncdb-relay-{name}"));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        TempDir(dir)
    }

    /// A new handle on the relay database in this directory.
    pub fn store(&self) -> SqliteStore {
        SqliteStore::open(&self.0.join("relay.sqlite")).expect("the database opens")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The relay's configuration in the tests: the simulated account may
/// create spaces, and the clock stands still.
pub fn config() -> Config {
    let mut config = Config::new(BTreeSet::from([account().verifying_key().to_bytes()]));
    config.clock = || 1_700_000_000_000;
    config
}

/// A relay on a fresh SQLite database, and the directory that holds it.
pub fn relay() -> (Relay<SqliteStore>, TempDir) {
    let dir = TempDir::new();
    (Relay::new(dir.store(), config()), dir)
}

/// `uploader` pushes these changes.
pub fn push(
    relay: &Relay<SqliteStore>,
    uploader: Who,
    space: SpaceId,
    changes: &[&Change],
) -> Vec<Pushed> {
    let frames = changes.iter().map(|c| c.bytes().to_vec()).collect();
    block_on(relay.push(id(uploader), space, frames)).results
}

/// `uploader` pushes one change.
pub fn push_one(relay: &Relay<SqliteStore>, uploader: Who, change: &Change) -> Pushed {
    let space = change.header().space;
    push(relay, uploader, space, &[change]).remove(0)
}
