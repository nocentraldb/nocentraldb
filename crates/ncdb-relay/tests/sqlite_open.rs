//! Several handles may open one new database at the same time (NOC-137).

use std::path::PathBuf;
use std::sync::Barrier;
use std::thread;

use ncdb_relay::sqlite::SqliteStore;

/// A temporary directory, removed on drop.
struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fresh_dir() -> TempDir {
    let mut name = [0u8; 16];
    getrandom::fill(&mut name).expect("a secure generator");
    let name: String = name.iter().map(|b| format!("{b:02x}")).collect();
    let dir = std::env::temp_dir().join(format!("ncdb-relay-open-{name}"));
    std::fs::create_dir_all(&dir).expect("a temporary directory");
    TempDir(dir)
}

/// Opening a new database from several handles at once switches it to WAL
/// while the others hold locks; every handle still opens it. The race is
/// rare in one round, so the test runs many rounds, each on a new database.
#[test]
fn handles_opening_a_new_database_at_once_all_open_it() {
    const ROUNDS: usize = 200;
    const HANDLES: usize = 8;
    for round in 0..ROUNDS {
        let dir = fresh_dir();
        let path = dir.0.join("relay.sqlite");
        let start = Barrier::new(HANDLES);
        thread::scope(|scope| {
            for _ in 0..HANDLES {
                scope.spawn(|| {
                    start.wait();
                    if let Err(error) = SqliteStore::open(&path) {
                        panic!("round {round}: the database does not open: {error:?}");
                    }
                });
            }
        });
    }
}
