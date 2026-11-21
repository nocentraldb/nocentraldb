//! The import on the first device: existing records written into one
//! space, `batch` creates to a change, then pushed to the relay.

use std::time::{Duration, Instant};

use nocentraldb::{Db, ObjectId, Space, Value};

use crate::device;

/// What the import measured.
#[derive(Debug)]
pub struct Imported {
    pub space: Space,
    /// The last item created, which a device that holds it holds all.
    pub last: ObjectId,
    /// The changes the import wrote.
    pub changes: u64,
    /// From the first write until every change was folded on the device.
    pub written: Duration,
    /// From the first write until the relay held every change.
    pub at_relay: Duration,
}

/// The fields of the `i`th imported record, as a record of a task
/// tracker has them: a title of about 50 characters, a status, a priority,
/// an estimate, a flag and two tags.
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

/// Writes `operations` records into a new space of `db`, `batch` creates
/// to one change, and waits until the relay holds them all.
pub async fn run(db: &Db, operations: u64, batch: u64) -> Result<Imported, String> {
    let space = db.spaces().create().await.map_err(|e| e.to_string())?;
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
    let written = start.elapsed();
    device::synced(db).await?;
    let at_relay = start.elapsed();
    Ok(Imported {
        space,
        last: last.ok_or("nothing to import")?,
        changes,
        written,
        at_relay,
    })
}
