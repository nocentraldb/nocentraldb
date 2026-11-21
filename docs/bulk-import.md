# Bursts of writes: imports and devices coming back

Most writes come one at a time, as people and agents work. Some come in a burst: an app moves to NoCentralDB and imports its existing records, often tens of thousands for one account, or a device that worked offline for days comes back with everything it wrote. This guide shows how to write a burst with the Rust SDK, and what it costs at 81,000 operations into one space: how long it takes, what the relay receives, and how the other devices catch up.

The numbers come from [crates/nocentraldb/examples/bulk_import](../crates/nocentraldb/examples/bulk_import/main.rs), which writes synthetic records and which you can run yourself ([Measure it yourself](#measure-it-yourself)). It uses only the public SDK and the local relay of [the Rust guide](getting-started.md), step 8. The guide assumes you know that guide's steps 1 to 4 and 8.

## Write a burst in batches

There is no import call. A burst is ordinary writes, and each `space.write` becomes one change: signed, encrypted, and received by other devices whole or not at all. So the one decision a burst makes is how many operations go into one write. Write them in batches of a few hundred, and wait until the relay holds them:

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

Each write returns once its change is folded on the device, so reads and queries see the new records at once, before the relay holds them. If the relay cannot be reached, the writes stay on the device and sync once it can, as every write does: a device writing offline batches the same way.

If an import stops halfway, the batches written so far stay: on the device, and on the relay once they synced. The SDK does not know which records came from where. To resume, the app keeps track of how far it got, such as the number of the last batch it wrote, and starts after it.

## How many operations to a change

Each operation of the measurement creates a record of a task tracker: a title of about 50 characters, a status, a priority, an estimate, a flag and two tags. One create takes about 390 bytes of a change. In this table, the first device writes 81,000 of them online, and then a new device of the same account joins and catches up. Each row is one run on the machine described [below](#what-was-measured); runs of the same row varied by up to 15%.

| Operations per change | Changes | Written on the device | Held by the relay | Relay receives | New device caught up | Peak memory: writer, new device, relay |
| --------------------: | ------: | --------------------: | ----------------: | -------------: | -------------------: | -------------------------------------: |
|                     1 |  81,000 |                 144 s |             208 s |        56.8 MB |                 92 s |                  426 MB, 478 MB, 11 MB |
|                    50 |   1,620 |                  39 s |              62 s |        32.2 MB |                 20 s |                  164 MB, 216 MB, 12 MB |
|                   500 |     162 |                  32 s |              37 s |        31.6 MB |                 16 s |                  185 MB, 216 MB, 17 MB |
|                 5,000 |      17 |                  28 s |              29 s |        31.5 MB |                 18 s |                  308 MB, 267 MB, 37 MB |
|                40,000 |       3 |                  30 s |              30 s |        31.5 MB |                 19 s |               1,383 MB, 716 MB, 165 MB |

"Held by the relay" counts from the first write, so it includes the writing.

What the numbers say:

- **One operation to a change is the slow way.** Each change carries its own signature, header and encryption, about 310 bytes, so the relay receives almost twice the bytes, and every step takes four to six times as long.
- **Above a few hundred operations a change, larger changes gain little.** At 500 operations a change the relay held the burst after 37 seconds, at 5,000 after 29. Most of the time is the device folding the records into its SQLite view. 500 operations a change is a good default: about 195 KB a change, with moderate memory.
- **Memory grows with the size of a change.** The writer, the relay and every device that receives a change hold it while they encode, check or fold it. Each MB more in the largest change raised the writer's peak memory by about 75 MB, the new device's by about 30 MB and the relay's by about 10 MB. At 40,000 operations a change, 15.5 MB, the writer peaked at 1.4 GB. Keep changes small enough for the smallest device that will receive them, such as a phone or a browser tab. Whether the SDK can hold less is open (NOC-422).
- **A change above 22 KiB goes through an upload.** The relay's messages are at most 23 KiB, so a larger change goes up through an upload URL, and other devices download it ([RFC 0007, "Large changes"](rfc/0007-relay.md#large-changes)). The SDK does that for you. In the measurement, uploads cost no time: 500 operations a change, each uploaded, was faster than 50, each inline.

### A change above 16 MiB never leaves the device

The relay accepts no change larger than 16 MiB ([RFC 0007, "Large changes"](rfc/0007-relay.md#large-changes)). The SDK does not check that limit when you write. A larger write succeeds and is folded on the device, but the relay refuses it with `too_large`, and it stays on that device only: no other device ever receives it. The only sign is `refused` in `db.sync().current()`, which the example above checks. At about 390 bytes an operation, the limit is about 43,000 operations in one change. In the measurement, 60,000 operations in one write made a change of 23 MB, which the relay refused. Whether the SDK should refuse such a write itself is open (NOC-421).

So never write a whole burst in one change. Keep the batch size well below the limit, and check `refused` afterwards.

## A device that comes back after working offline

Island mode lets members keep working without a relay. When a device comes back, its burst goes up, and the other devices pull it. In this table, a second device of the same account joined the space and went offline. The first device then wrote the 81,000 operations offline, reconnected and pushed them. Then the second device came back and pulled them.

| Operations per change | Written offline | Held by the relay, from reconnecting | Second device caught up, from reopening | Peak memory: writer, second device, relay |
| --------------------: | --------------: | -----------------------------------: | --------------------------------------: | ----------------------------------------: |
|                     1 |           134 s |                                 44 s |                                   133 s |                     511 MB, 286 MB, 12 MB |
|                    50 |            39 s |                                2.3 s |                                    35 s |                     213 MB, 116 MB, 12 MB |
|                   500 |            32 s |                                1.4 s |                                    25 s |                     207 MB, 126 MB, 18 MB |
|                 5,000 |            29 s |                                1.2 s |                                    21 s |                     286 MB, 194 MB, 30 MB |

- **Writing offline costs what writing online costs.** The time is the device folding the records, with or without a relay.
- **Pushing a batched burst takes seconds.** Once the device reconnects, its changes are already signed and encrypted. At 500 operations a change, the relay held all 81,000 operations 1.4 seconds after the reconnect. 81,000 separate changes took 44 seconds; a push carries at most 32 changes, so that took more than 2,500 pushes.
- **The returning device pulls like a new one.** It received the whole burst and folded it. In this measurement it took longer than a new device joining after the burst, 25 seconds against 16 to 18 at 500 operations a change, and held less memory. This guide did not find out why.

A burst does not change how changes merge: changes written offline merge with what the others wrote meanwhile, as any concurrent changes do, and a change that loses to one its author had not seen stays identifiable as such.

## What the relay receives

The relay stores what the burst sends and cannot read it. At 500 operations a change, it received 162 changes of up to 195 KB each, 31.6 MB in all, plus five small changes the SDK wrote to set up the account and the space. Its SQLite database grew to 36 MB, a little more than the encrypted records: about 450 bytes for each operation.

The local relay sets no limit on how fast a member writes. What it took is in the tables: at 500 operations a change, about 2,200 operations a second from the first write until the relay held them, most of it the writer folding; at one operation a change, about 390 changes a second. A device coming back pushed 162 changes in 1.4 seconds.

What the relay sees is the metadata ([The relay](https://nocentraldb.io/docs/concepts/relay/)): one member key wrote 162 changes of these sizes into one space within seconds. A burst is visible to the relay as a burst, and its size shows roughly how many records it held. Their content does not.

On AWS, each change is one append to the space's log in DynamoDB, and each change above 22 KiB is also an object in S3 ([RFC 0007, "How large changes go to S3"](rfc/0007-relay.md#how-large-changes-go-to-s3)). Fewer, larger changes mean fewer appends. This guide measured the local relay only.

## How a new device catches up

A device that joins the space later, whether a paired device of the same account or another member added with `members().add`, receives the whole log of the space and folds it into its own view. In the measurement, a new device of the same account was paired after the burst ([the Rust guide](getting-started.md), step 9). It held all 81,000 records 16 to 20 seconds later, for any batch size from 50 to 40,000. At one operation a change it took 92 seconds.

Every copy of the space takes space on disk, on each device:

- **About 2.3 KB a record.** Both devices' databases ended at 184 MB for the 81,000 records. That is more than the encrypted records, 32 MB of which the device also keeps as its copy of the log. Most of the rest is each field's merge state, which lets concurrent edits merge, and the indexes.
- **Up to twice that while a new device catches up.** On the new device, SQLite's write-ahead log grew to 187 MB next to the database while it folded the burst. SQLite folds it back into the database; after the database closed, the file was 184 MB and the log was gone. The device that came back after working offline did not show this: its log stayed under 60 MB.

## What was measured

Each row of the tables is one run of the example: 81,000 operations into one space, a first and a second device of the same account, both on the Rust SDK, both syncing with the local relay `ncdb-relay` on SQLite over the loopback interface. It ran on one Linux machine with 4 virtual CPUs (Intel Xeon, 2.30 GHz) and 16 GB of memory, in a release build. Both devices and the relay shared that machine.

What it does not show:

- **A network.** The relay ran on the same machine. Over the internet, each push and pull also waits for the round trip, and the bytes of the tables travel over your connection.
- **The relay on AWS.** API Gateway, DynamoDB and S3 have their own latency and cost. This guide did not measure them.
- **A browser or a phone.** The TypeScript SDK in a browser stores its database in the origin private file system, where writes are slower, and a tab has less memory than a process. Neither was measured. The memory columns are the best guide to what a smaller device must hold.
- **Other records, or edits.** Every operation here creates a record. Larger records make larger changes: with larger records, put fewer to a change, so that each change stays a few hundred KB. Updates and deletes were not measured.
- **Several writers at once.** One device wrote the burst. Bursts from several members into one space at the same time were not measured.

## Measure it yourself

From the root of a clone of the repository, build the relay and run the example:

```sh
cargo build --release --locked -p ncdb-relay-server --bin ncdb-relay
cargo run --release --locked -p nocentraldb --example bulk_import -- \
  --operations 81000 --batch 500
```

`--operations` is the number of operations, `--batch` the operations to a change, and `--island` runs the second table's case instead of the first's. `--relay-bin` names the relay binary if it is not in the same target directory, and `--dir` a new directory that keeps the databases after the run, for a look inside them. Without `--dir`, the run uses a temporary directory and removes it. One run at 81,000 operations takes about a minute; at one operation a change, about five.

The repository's tests run both cases at 2,000 operations, 50 to a change, so that the example keeps working ([crates/nocentraldb/tests/bulk_import.rs](../crates/nocentraldb/tests/bulk_import.rs)). The runs at full size are not part of the gate, because they take minutes. Run them by hand after a change that could affect writing, syncing or folding, and update the tables here when the numbers change.
