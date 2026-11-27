//! What the log of an open space keeps in memory (NOC-452). A device
//! keeps the frames of a space's changes in its database; in memory the
//! log keeps what every write and every received change needs: each
//! change's id, header without its payload, state and relay bookkeeping,
//! the tips, and the control changes, which the membership checks hold
//! anyway. A sealed change's bytes are read back from the database when a
//! rare pass needs them. So the memory of an open space grows with its
//! number of changes by a small amount each, not with their bytes. SQLite's
//! own memory is not counted here: it allocates outside Rust's allocator.

mod support;

#[path = "support/peak.rs"]
mod peak;

use nocentraldb::{Db, OpenOptions, Value};
use support::{LOCK, TempDir, now};

#[global_allocator]
static COUNTING: peak::Counting = peak::Counting;

/// The fields of the `i`th record, as the bulk import measurement writes
/// them (`examples/bulk_import`): about 390 bytes of a change each.
fn fields(i: u64) -> [(&'static str, Value); 6] {
    let status = if i.is_multiple_of(3) { "done" } else { "open" };
    [
        (
            "title",
            Value::from(format!("Imported task {i:06}: follow up on the old record")),
        ),
        ("status", Value::from(status)),
        ("priority", Value::from((i % 5) as i64)),
        ("estimate", Value::from(0.5 * (i % 8) as f64)),
        ("urgent", Value::from(i.is_multiple_of(10))),
        (
            "tags",
            Value::add(["imported", if i.is_multiple_of(2) { "even" } else { "odd" }]),
        ),
    ]
}

/// The bytes of a change for one record of [`fields`], as the bulk import
/// measured them (docs/bulk-import.md).
const CHANGE_BYTES_PER_RECORD: usize = 390;

/// Records per change, as the bulk import guide recommends.
const BATCH: u64 = 500;

/// Writes `changes` changes of [`BATCH`] records each into `space`.
fn write(space: &nocentraldb::Space, changes: u64) {
    for change in 0..changes {
        now(space.write(|tx| {
            for i in 0..BATCH {
                tx.create("tasks", fields(change * BATCH + i))?;
            }
            Ok(())
        }))
        .unwrap();
    }
}

/// Once written and folded, a change's bytes stay in the database, not in
/// memory: twenty changes of 195 KB leave far less than their bytes on the
/// heap.
#[test]
fn written_changes_are_not_kept_in_memory() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let space = now(db.spaces().create()).unwrap();
    write(&space, 1);
    let changes = 20;
    let ((), held) = peak::peak(|| write(&space, changes));
    let bytes = (changes * BATCH) as usize * CHANGE_BYTES_PER_RECORD;
    assert!(
        held.kept <= bytes / 20,
        "{changes} changes of {bytes} bytes in all kept {} bytes on the heap",
        held.kept
    );
}

/// A database opened again reads every frame once, and keeps none of
/// their bytes.
#[test]
fn a_reopened_database_keeps_no_change_bytes() {
    let dir = TempDir::new("memory-log");
    let options = || {
        OpenOptions::new(LOCK)
            .path(dir.join("data"))
            .key_file(dir.join("member.key"))
    };
    let changes = 20;
    {
        let db = now(Db::open(options())).unwrap();
        let space = now(db.spaces().create()).unwrap();
        write(&space, changes);
    }
    let (db, held) = peak::peak(|| now(Db::open(options())).unwrap());
    let bytes = (changes * BATCH) as usize * CHANGE_BYTES_PER_RECORD;
    assert!(
        held.kept <= bytes / 20,
        "a database of {bytes} change bytes kept {} bytes on the heap at open",
        held.kept
    );
    drop(db);
}

/// What an open space keeps for each change it holds, whatever the
/// change's size, measured after a reopen with changes of one record
/// each, where it is most of what the space keeps (NOC-518).
const BOOKKEEPING_PER_CHANGE: usize = 600;

/// A space that holds many small changes keeps a few hundred bytes for
/// each of them once it is opened again: its id, header, state, position,
/// sequence and dependencies (NOC-518).
#[test]
fn a_reopened_space_keeps_little_per_change() {
    let dir = TempDir::new("memory-log-small");
    let options = || {
        OpenOptions::new(LOCK)
            .path(dir.join("data"))
            .key_file(dir.join("member.key"))
    };
    let changes = 2_000;
    let space = {
        let db = now(Db::open(options())).unwrap();
        let space = now(db.spaces().create()).unwrap();
        for i in 0..changes {
            now(space.write(|tx| tx.create("tasks", fields(i)).map(drop))).unwrap();
        }
        space.id()
    };
    let (db, held) = peak::peak(|| {
        let db = now(Db::open(options())).unwrap();
        let space = now(db.spaces().get(space)).unwrap();
        now(space.write(|tx| tx.create("tasks", fields(changes)).map(drop))).unwrap();
        db
    });
    let per_change = held.kept / changes as usize;
    assert!(
        per_change <= BOOKKEEPING_PER_CHANGE,
        "a space of {changes} changes of one record kept {per_change} bytes per change \
         on the heap after a reopen, more than {BOOKKEEPING_PER_CHANGE}"
    );
    drop(db);
}
