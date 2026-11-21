//! The burst on the first device: synthetic records written into one
//! space, `batch` creates to a change.

use std::time::{Duration, Instant};

use nocentraldb::{ObjectId, Space, Value};

/// What the writes measured.
#[derive(Debug)]
pub struct Written {
    /// The last item created, which a device that holds it holds all.
    pub last: ObjectId,
    /// The changes written.
    pub changes: u64,
    /// From the first write until every change was folded on the device.
    pub took: Duration,
}

/// The fields of the `i`th record, as a record of a task tracker has them:
/// a title of about 50 characters, a status, a priority, an estimate, a
/// flag and two tags.
pub fn fields(i: u64) -> [(&'static str, Value); 6] {
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

/// Writes `operations` records into `space`, `batch` creates to one
/// change.
pub async fn write(space: &Space, operations: u64, batch: u64) -> Result<Written, String> {
    let start = Instant::now();
    let (mut changes, mut last) = (0, None);
    let mut next = 0;
    while next < operations {
        let end = (next + batch).min(operations);
        let written = space
            .write(|tx| {
                for i in next..end {
                    tx.create("tasks", fields(i))?;
                }
                Ok(())
            })
            .await
            .map_err(|e| format!("write {next}..{end}: {e}"))?;
        last = written.ids.last().copied();
        changes += 1;
        next = end;
    }
    Ok(Written {
        last: last.ok_or("nothing to write")?,
        changes,
        took: start.elapsed(),
    })
}
