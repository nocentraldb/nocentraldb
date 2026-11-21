# Importing existing data in bulk

An app that moves to NoCentralDB brings its records along: often tens of thousands of them for one account. This guide shows how to write them into a space in bulk with the Rust SDK, and what that costs at 81,000 records: how long the import takes, what the relay receives, and how a second device catches up.

The numbers come from [crates/nocentraldb/examples/bulk_import](../crates/nocentraldb/examples/bulk_import/main.rs), which you can run yourself ([Measure it yourself](#measure-it-yourself)). It uses only the public SDK and the local relay of [the Rust guide](getting-started.md), step 8. The guide assumes you know that guide's steps 1 to 4 and 8.

## Write the records in batches

There is no import call. An import is ordinary writes, and each `space.write` becomes one change: signed, encrypted, and received by other devices whole or not at all. So the one decision an import makes is how many records go into one write. Write them in batches of a few hundred, and wait until the relay holds the import:

```rust no_run
use futures_util::StreamExt;
use nocentraldb::{Db, OpenOptions, Value};

/// A record of the system the app moves from.
struct Record {
    title: String,
    done: bool,
}

/// The records to import, read from the old system.
fn old_records() -> Vec<Record> {
    Vec::new()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let options = OpenOptions::new(&lock)
        .path("data")
        .relay("ws://127.0.0.1:7700");
    let db = Db::open(options).await?;
    let space = db.spaces().create().await?;

    // 500 records to a write: one change each.
    for batch in old_records().chunks(500) {
        space
            .write(|tx| {
                for record in batch {
                    let status = if record.done { "done" } else { "open" };
                    tx.create(
                        "tasks",
                        [("title", Value::from(record.title.clone())), ("status", status.into())],
                    )?;
                }
                Ok(())
            })
            .await?;
    }

    // The import is on this device now. Wait until the relay holds it.
    let mut sync = db.sync().subscribe();
    while let Some(status) = sync.next().await {
        if status.refused > 0 {
            return Err("the relay refused part of the import".into());
        }
        if status.unconfirmed == 0 {
            break;
        }
    }
    Ok(())
}
```

Each write returns once its change is folded on the device, so reads and queries see the imported records at once, before the relay holds them. If the relay cannot be reached, the import stays on the device and syncs later, as every write does.

If the import stops halfway, the batches written so far stay: on the device, and on the relay once they synced. The SDK does not know which records came from where. To resume, the app keeps track of how far it got, such as the number of the last batch it wrote, and starts after it.

## How many records to a change

At 81,000 records of a task tracker (a title of about 50 characters, a status, a priority, an estimate, a flag and two tags), one create takes about 390 bytes of a change. Measured on the machine described [below](#what-was-measured), with the first device writing and a second device catching up:

| Records per change | Changes | Written on the device | Held by the relay | Relay receives | Second device caught up | Peak memory: writer, second device, relay |
| -----------------: | ------: | --------------------: | ----------------: | -------------: | ----------------------: | ----------------------------------------: |
|                  1 |  81,000 |                 144 s |             208 s |        56.8 MB |                    92 s |                  426 MB, 478 MB, 11 MB |
|                 50 |   1,620 |                  39 s |              62 s |        32.2 MB |                    20 s |                  164 MB, 216 MB, 12 MB |
|                500 |     162 |                  32 s |              37 s |        31.6 MB |                    16 s |                  185 MB, 216 MB, 17 MB |
|              5,000 |      17 |                  28 s |              29 s |        31.5 MB |                    18 s |                  308 MB, 267 MB, 37 MB |
|             40,000 |       3 |                  30 s |              30 s |        31.5 MB |                    19 s |              1,383 MB, 716 MB, 165 MB |

What the numbers say:

- **One record to a change is the slow way.** Each change carries its own signature, header and encryption, about 310 bytes, so the relay receives almost twice the bytes, and every step takes four to six times as long.
- **Above a few hundred records a change, larger changes gain little.** At 500 records a change the relay held the import after 37 seconds, at 5,000 after 29. Most of the time is the device folding the records into its SQLite view. 500 records a change is a good default: about 195 KB a change, with moderate memory.
- **Memory grows with the size of a change.** The writer, the relay and every device that receives a change hold it while they encode, check or fold it. Each MB more in the largest change raised the writer's peak memory by about 75 MB, the second device's by about 30 MB and the relay's by about 10 MB. At 40,000 records a change, 15.5 MB, the writer peaked at 1.4 GB. Keep changes small enough for the smallest device that will receive them, such as a phone or a browser tab.
- **A change above 22 KiB goes through an upload.** The relay's messages are at most 23 KiB, so a larger change goes up through an upload URL, and other devices download it ([RFC 0007, "Large changes"](rfc/0007-relay.md#large-changes)). The SDK does that for you. In the measurement, uploads cost no time: 500 records a change, each uploaded, was faster than 50, each inline.

### A change above 16 MiB never leaves the device

The relay accepts no change larger than 16 MiB ([RFC 0007, "Large changes"](rfc/0007-relay.md#large-changes)). The SDK does not check that limit when you write. A larger write succeeds and is folded on the device, but the relay refuses it with `too_large`, and it stays on that device only: no other device ever receives it. The only sign is `refused` in `db.sync().current()`, which the example above checks. At about 390 bytes a record, the limit is about 43,000 records in one change. In the measurement, 60,000 records in one write made a change of 23 MB, which the relay refused.

So never write a whole import in one change. Keep the batch size well below the limit, and check `refused` after the import.

## What the relay receives

The relay stores what the import sends and cannot read it. At 500 records a change, it received 162 changes of up to 195 KB each, 31.6 MB in all, plus five small changes the SDK wrote to set up the account and the space. Its SQLite database grew to 36 MB, a little more than the encrypted records: about 450 bytes for each record you import.

What the relay sees is the metadata ([The relay](https://nocentraldb.io/docs/concepts/relay/)): one member key wrote 162 changes of these sizes into one space within about 40 seconds. A bulk import is visible to the relay as a bulk import, and its size shows roughly how many records it held. Their content does not.

On AWS, each change is one append to the space's log in DynamoDB, and each change above 22 KiB is also an object in S3 ([RFC 0007, "How large changes go to S3"](rfc/0007-relay.md#how-large-changes-go-to-s3)). Fewer, larger changes mean fewer appends. This guide measured the local relay only.

## How a second device catches up

A device that joins the space later, whether a paired device of the same account or another member added with `members().add`, receives the whole log of the space and folds it into its own view. In the measurement, a second device of the same account was paired after the import ([the Rust guide](getting-started.md), step 9). It held all 81,000 records 16 to 20 seconds later, for any batch size from 50 to 40,000. At one record a change it took 92 seconds.

Every copy of the space takes space on disk, on each device:

- **About 2.3 KB a record.** Both devices' databases ended at 184 MB for the 81,000 records. That is more than the encrypted records, 32 MB of which the device also keeps as its copy of the log. Most of the rest is each field's merge state, which lets concurrent edits merge, and the indexes.
- **Up to twice that while it catches up.** On the second device, SQLite's write-ahead log grew to 187 MB next to the database while it folded the import. SQLite folds it back into the database; after the database closed, the file was 184 MB and the log was gone.

## What was measured

Each row of the table is one run of the example: 81,000 records into one new space on one device, then a second device of the same account paired, both on the Rust SDK, both syncing with the local relay `ncdb-relay` on SQLite over the loopback interface. It ran on one Linux machine with 4 virtual CPUs (Intel Xeon, 2.30 GHz) and 16 GB of memory, in a release build. Both devices and the relay shared that machine.

What it does not show:

- **A network.** The relay ran on the same machine. Over the internet, each push and pull also waits for the round trip, and the bytes of the table travel over your connection.
- **The relay on AWS.** API Gateway, DynamoDB and S3 have their own latency and cost. This guide did not measure them.
- **A browser or a phone.** The TypeScript SDK in a browser stores its database in the origin private file system, where writes are slower, and a tab has less memory than a process. Neither was measured. The memory column above is the best guide to what a smaller device must hold.
- **Other records.** Larger records make larger changes. Measure your own: with fewer records to a change if yours are larger, so that each change stays a few hundred KB.

## Measure it yourself

From the root of a clone of the repository, build the relay and run the example:

```sh
cargo build --release --locked -p ncdb-relay-server --bin ncdb-relay
cargo run --release --locked -p nocentraldb --example bulk_import -- \
  --operations 81000 --batch 500
```

`--operations` is the number of records, `--batch` the records to a change, `--relay-bin` the relay binary if it is not in the same target directory, and `--dir` a new directory that keeps the databases after the run, for a look inside them. Without `--dir`, the run uses a temporary directory and removes it. One run at 81,000 records takes about a minute; at one record a change, about five.

The repository's tests run the example at 2,000 records, 50 to a change, so that it keeps working ([crates/nocentraldb/tests/bulk_import.rs](../crates/nocentraldb/tests/bulk_import.rs)). The run at full size is not part of the gate, because it takes minutes. Run it by hand after a change that could affect writing, syncing or folding, and update the table here when the numbers change.
