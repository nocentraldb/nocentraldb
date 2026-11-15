# Getting started with the Rust SDK

This guide builds a small task list on NoCentralDB's Rust SDK, the crate `nocentraldb`. You start with one database on one device. Then you share a space with a second device through a relay that cannot read it, pair a second device of your own account, and keep the account's recovery code.

NoCentralDB is not released yet. The SDK works, and this guide uses only what is merged, but names and formats can still change. Every Rust snippet below is compiled by the repository's tests, so the guide cannot drift from the SDK.

For a finished app that does all of this from the command line, see [apps/tasks-rust](../apps/tasks-rust/README.md). [RFC 0009](rfc/0009-sdk-api.md) describes every call. For the TypeScript SDK in a web page, see [the browser guide](getting-started-browser.md).

## What you need

- A recent stable Rust. The repository tests with the version in `rust-toolchain.toml`.
- Node.js 24 or later and pnpm, to build the model's lock.
- A clone of this repository. Until the first release, you build your app inside it, in a folder under `apps/`, as [apps/tasks-rust](../apps/tasks-rust/README.md) does: the schema build and the SDK come from the clone.

## 1. Make the app

Make a folder `apps/my-tasks` in the clone. Its `Cargo.toml` takes the SDK from the workspace, tokio to run its async calls, and futures-util to read its streams, at the versions the repository already pins:

```toml
[package]
name = "my-tasks"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
nocentraldb.workspace = true
tokio = { version = "=1.54.1", default-features = false, features = ["rt", "macros"] }
futures-util = { version = "=0.3.34", default-features = false, features = ["std"] }
```

The folder joins the clone's Cargo workspace, which needs a target in every member, so give it a `src/main.rs` at once, even an empty `fn main() {}`; the programs below replace it.

Its `package.json` lets the schema build import the TypeScript SDK from the clone:

```json
{
  "name": "my-tasks",
  "private": true,
  "type": "module",
  "dependencies": { "@nocentraldb/sdk": "workspace:*" }
}
```

## 2. Write the model and build its lock

You write the schema once, in TypeScript, in `apps/my-tasks/model.ts`. This one has one collection, `tasks`, with a title and a status, and one index, `byStatus`, that reads the tasks of one status in title order:

```ts
import { nc } from "@nocentraldb/sdk";

export const model = nc.model({
  tasks: nc.collection(
    {
      title: nc.string(),
      status: nc.oneOf(["open", "done"]),
    },
    {
      indexes: {
        byStatus: nc.index(["status", "title"], { equal: 1 }),
      },
    },
  ),
});
```

The SDK never reads `model.ts`. It reads `model.lock.json`, which `ncdb model build` writes from it. From the root of the clone, install the packages and build the SDK's TypeScript once, then build the lock in the app's folder:

```sh
pnpm install
node packages/sdk/build.mjs --ts-only
cd apps/my-tasks && cargo run -q -p ncdb -- model build
```

Commit `model.lock.json` with your code. Build it again after every change to the schema. The build refuses a change that devices on the old lock could not keep, such as renaming a field.

Each program below is a `src/main.rs` of the app. Run it with `cargo run` in `apps/my-tasks`, where it finds `model.lock.json`.

## 3. Open a database

A database lives in a directory the SDK owns. Its keys live in the platform's keystore: the Keychain on macOS, the Secret Service on Linux, the Credential Manager on Windows.

```rust no_run
use nocentraldb::{Db, OpenOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
    println!("spaces: {}", db.spaces().list().await?.len());
    Ok(())
}
```

A machine without a keystore, such as a server or a container, keeps the keys in a file you name with `.key_file("data.keys")`. Only the file's permissions protect it. For tests, `.in_memory()` keeps the database and its keys in memory.

Every call runs on the device. A read never waits for the network.

## 4. Create a space and write

Objects live in a space. A space is what you share: its members read and write it, and nobody else can. A write is one change in one space, signed and encrypted, and resolves when it is saved on the device.

```rust no_run
use nocentraldb::{Db, OpenOptions, Value};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;

    let project = db.spaces().create().await?;
    let written = project
        .write(|tx| {
            tx.create(
                "tasks",
                [("title", Value::from("Write the README")), ("status", "open".into())],
            )?;
            tx.create(
                "tasks",
                [("title", Value::from("Buy milk")), ("status", "open".into())],
            )?;
            Ok(())
        })
        .await?;
    println!("space {}, tasks {:?}", project.id(), written.ids);

    // One create, update or delete is a write of its own.
    let task = written.ids[0];
    project.update("tasks", task, [("status", "done".into())]).await?;
    Ok(())
}
```

Both creates in the `write` are one change: other devices receive both or neither.

## 5. Read an object by its id

```rust no_run
use nocentraldb::{Db, ObjectId, OpenOptions, SpaceId};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
    let space: SpaceId = std::env::args().nth(1).ok_or("a space id")?.parse()?;
    let id: ObjectId = std::env::args().nth(2).ok_or("a task id")?.parse()?;

    let project = db.spaces().get(space).await?;
    if let Some(task) = project.get("tasks", id).await? {
        let title: Option<&str> = task.get("title")?;
        println!("{} (deleted: {})", title.unwrap_or(""), task.deleted());
    }
    Ok(())
}
```

A field you never wrote reads as `None`. A deleted object is still read by its id, so you can show it or restore it.

## 6. Query through an index

A query reads a declared index: the rows with given values for its leading fields, in index order, a page at a time. There is no other query, so no read can scan a whole table.

```rust no_run
use nocentraldb::{Db, OpenOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;

    let open = db.index("tasks", "byStatus")?.equal(["open"]).take(20);
    let mut page = db.find(&open).await?;
    loop {
        for task in &page.items {
            println!("[ ] {}", task.get::<&str>("title")?.unwrap_or(""));
        }
        match page.cursor {
            Some(cursor) if page.has_more => {
                page = db.find(open.clone().after(cursor)).await?;
            }
            _ => break,
        }
    }
    Ok(())
}
```

`db.find` reads every space of the database. An index declared with `scope: "space"` reads one space at a time, through `space.find`.

## 7. Follow a query

A subscription gives the query's page at once, and again after every write that touched the collection, whether it came from this device or from sync.

```rust no_run
use futures_util::StreamExt;
use nocentraldb::{Db, OpenOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("data")).await?;

    let open = db.index("tasks", "byStatus")?.equal(["open"]).take(50);
    let mut pages = db.subscribe(open);
    while let Some(page) = pages.next().await {
        println!("{} open tasks", page?.items.len());
    }
    Ok(())
}
```

Drop the stream to stop following.

## 8. Two devices through a relay

The relay stores and forwards changes. It holds them encrypted and cannot read them. Run the local relay from the clone, and let the laptop's account create spaces on it. The relay names the account by the laptop's invitation, which the next program prints on any device, so no key is shown:

```sh
cargo run -q -p ncdb-relay-server --bin ncdb-relay -- \
  --database relay.sqlite --listen 127.0.0.1:7700 \
  --allow-invitation "ncdb-invite-1:…"
```

Each device opens its own database with the relay's URL. The SDK syncs in the background and keeps working offline. On the phone, print its invitation. It holds the device's public keys and its account's fingerprint, and no secret, so send it any way you like:

```rust no_run
use nocentraldb::{Db, OpenOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let options = OpenOptions::new(&lock)
        .path("phone")
        .relay("ws://127.0.0.1:7700");
    let db = Db::open(options).await?;
    let invitation = db.invitation().await;
    println!("{invitation}");
    println!("fingerprint: {}", invitation.fingerprint());
    Ok(())
}
```

On the laptop, add the phone to a space. The first time, the phone's account is new to the laptop: a person compares the fingerprint on both screens, the way messaging apps compare safety numbers, and only then does the app pass it. Then the laptop writes a task for the phone and waits until the relay holds it:

```rust no_run
use nocentraldb::{AddMember, Db, Invitation, OpenOptions, Role, SpaceId, Value};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let options = OpenOptions::new(&lock)
        .path("laptop")
        .relay("ws://127.0.0.1:7700");
    let db = Db::open(options).await?;
    let space: SpaceId = std::env::args().nth(1).ok_or("a space id")?.parse()?;
    let invitation: Invitation = std::env::args().nth(2).ok_or("an invitation")?.parse()?;

    // Only after a person compared this fingerprint with the phone's.
    let confirmed = invitation.fingerprint();
    let project = db.spaces().get(space).await?;
    let notice = project
        .members()
        .add(
            &invitation,
            AddMember {
                role: Role::Writer,
                fingerprint: Some(confirmed),
            },
        )
        .await?;
    println!("{}", notice.text);

    // A write resolves on the device; this waits until the relay holds it.
    let written = project
        .create(
            "tasks",
            [("title", Value::from("Shared with the phone")), ("status", "open".into())],
        )
        .await?;
    db.sync().confirmed(written.change).await;
    println!("{:?}", db.sync().current());
    Ok(())
}
```

`members.add` returns a notice. Its text says what the call does not do: here, that the new member reads the whole history of the space. The phone receives the space at its next sync, and a subscription on the phone shows the laptop's writes as they arrive.

`db.sync().current()` reports the connection, how many of this device's changes the relay has not confirmed, and the last error that stops syncing. `space.status().current()` reports what a space holds that a read does not show, such as changes that wait for a newer version of the app.

## 9. Pair a second device of your own account

A person's devices share one account. The account key stays on the person's devices. A new device opens its database for pairing and shows its invitation, as a QR code or text:

```rust no_run
use nocentraldb::{Db, OpenOptions, PairingAnswer};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("tablet").pairing()).await?;
    println!("{}", db.invitation().await);

    // The answer the laptop shows, once it paired this device.
    let answer: PairingAnswer = std::env::args().nth(1).ok_or("an answer")?.parse()?;
    for notice in db.account().complete_pairing(&answer).await? {
        println!("{}", notice.text);
    }
    Ok(())
}
```

A device that holds the account key pairs it, after a person scanned its code or compared its fingerprint on both screens. The answer carries the account key, sealed so that only the new device opens it:

```rust no_run
use nocentraldb::{Db, Invitation, OpenOptions, PairKind};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("laptop")).await?;
    let invitation: Invitation = std::env::args().nth(1).ok_or("an invitation")?.parse()?;

    let paired = db.account().pair(PairKind::Device, &invitation).await?;
    println!("{}", paired.answer);
    println!("added to {} spaces", paired.added.len());
    Ok(())
}
```

The laptop adds the new device to every space where it is an admin. `paired.ask_admins` lists the spaces whose admins your app asks to add it.

## 10. Keep the recovery code

The first open of an account creates its recovery code: twelve words. They restore the account if every device is lost. The SDK hands them to your app until the person confirms they stored them, then forgets them for good:

```rust no_run
use nocentraldb::{Db, OpenOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let lock = std::fs::read_to_string("model.lock.json")?;
    let db = Db::open(OpenOptions::new(&lock).path("laptop")).await?;
    if let Some(code) = db.account().recovery_code().await? {
        println!("Write these words down: {}", code.words().join(" "));
        // Only after the person said they stored them.
        code.confirm().await?;
    }
    Ok(())
}
```

The code prints as `RecoveryCode(redacted)`, so it never reaches a log by accident. On a new device, open a new database with `.recover("twelve words …")`. It keeps the account's fingerprint, and the other members add the device to their spaces again.

## Where to go next

- [apps/tasks-rust](../apps/tasks-rust/README.md) runs all of this from the command line, with a test that runs two devices end to end.
- [RFC 0009](rfc/0009-sdk-api.md) describes every call, error and report.
- [The FAQ](faq.md) answers what the relay sees, how conflicts merge and how the schema changes.
