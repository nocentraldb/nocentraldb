//! What one large write holds in memory (NOC-422). A write of thousands
//! of records records them, encodes them into one change, seals it and
//! folds it; the SDK must not hold the records' operations, or a tree of
//! them, several times at once, so that a change near the relay's limit
//! fits a phone or a browser tab. SQLite's own memory is not counted
//! here: the SDK's in-memory database allocates it outside Rust's
//! allocator.

mod support;

#[path = "support/peak.rs"]
mod peak;

use nocentraldb::Value;
use support::{now, open};

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

/// The write holds its operations, the sealed change and what the fold
/// decodes of it, each once: about eight times the change's bytes, where
/// it held fifteen while the payload's operations, a second decoding of
/// the change and a copy of it for the log stayed next to the fold.
#[test]
fn a_write_of_many_records_holds_a_few_times_its_changes_bytes() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let records = 2_000;
    let (written, held) = peak::peak(|| {
        now(space.write(|tx| {
            for i in 0..records {
                tx.create("tasks", fields(i))?;
            }
            Ok(())
        }))
    });
    written.unwrap();
    let change = records as usize * CHANGE_BYTES_PER_RECORD;
    assert!(
        held.peak <= 10 * change,
        "a write of {change} bytes held {:.1} times as many at its peak",
        held.peak as f64 / change as f64
    );
}
