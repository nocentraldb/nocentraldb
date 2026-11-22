//! The write-ahead log after a large write (NOC-422). SQLite folds the
//! log back into the database at every checkpoint, but keeps the file at
//! the largest size it reached until the database closes: after a burst,
//! such as a new device catching up, the log stood next to the database
//! at about its size. The view truncates it after each checkpoint to the
//! size it grows to between two checkpoints, 4 MiB.

mod support;

use nocentraldb::{Db, OpenOptions, Value};
use support::{LOCK, TempDir, now};

#[test]
fn after_a_large_write_the_write_ahead_log_is_at_most_4_mib() {
    let dir = TempDir::new("write-ahead-log");
    let options = OpenOptions::new(LOCK)
        .path(dir.join("db"))
        .key_file(dir.join("keys"));
    let db = now(Db::open(options)).unwrap();
    let space = now(db.spaces().create()).unwrap();
    // About 7 MB of pages in one transaction, and so in the log.
    for write in 0..2 {
        now(space.write(|tx| {
            for i in 0..3_000 {
                tx.create(
                    "tasks",
                    [
                        (
                            "title",
                            Value::from(format!("task {write} {i:06} of a burst")),
                        ),
                        ("priority", Value::from(i64::from(i % 5))),
                    ],
                )?;
            }
            Ok(())
        }))
        .unwrap();
    }
    let log =
        std::fs::metadata(dir.join("db").join("nocentraldb.sqlite-wal")).map_or(0, |m| m.len());
    assert!(
        log <= 4 * 1024 * 1024,
        "the write-ahead log holds {log} bytes after the writes"
    );
}
