# The landing page's Rust code

The Rust variant of each step of the landing page's code section
(`CodeSection.astro`), one section per step, named by the step's id. The doc
tests of `nocentraldb` compile every `rust` block (`crates/nocentraldb/src/lib.rs`),
so the page cannot drift from the SDK. Lines that start with `# ` are rustdoc's
hidden lines: they compile, and the page leaves them out.

## install

The line comes from `site/src/install.mjs` when the site is built.

```sh
{CARGO_ADD}
```

## open

```rust no_run
# use nocentraldb::{Db, OpenOptions};
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
let lock = std::fs::read_to_string("model.lock.json")?;
let options = OpenOptions::new(&lock)
    .path("data")
    .relay("wss://relay.example.com");
let db = Db::open(options).await?;

let project = db.spaces().create().await?;
# let _ = project;
# Ok(())
# }
```

## share

```rust no_run
# use nocentraldb::{AddMember, Db, Invitation, OpenOptions, Role};
# fn send(_: String) {}
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
# let lock = std::fs::read_to_string("model.lock.json")?;
# let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
# let project = db.spaces().create().await?;
# let text = String::new();
// On the joining member:
let invitation = db.invitation().await;
send(invitation.to_string());

// On an admin of the space:
let invitation: Invitation = text.parse()?;
let add = AddMember { role: Role::Writer, fingerprint: None };
project.members().add(&invitation, add).await?;
# Ok(())
# }
```

## write

```rust no_run
# use nocentraldb::{Db, OpenOptions, Value};
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
# let lock = std::fs::read_to_string("model.lock.json")?;
# let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
# let project = db.spaces().create().await?;
project
    .write(|tx| {
        let task = tx.create(
            "tasks",
            [("title", Value::from("Write the README")), ("status", "open".into())],
        )?;
        tx.update("tasks", task, [("status", "in_progress".into())])?;
        Ok(())
    })
    .await?;
# Ok(())
# }
```

## read

```rust no_run
# use nocentraldb::{Db, OpenOptions};
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
# let lock = std::fs::read_to_string("model.lock.json")?;
# let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
let query = db.index("tasks", "byStatusPriority")?.where_eq(["open"]).take(50);

let page = db.find(&query).await?;
for task in &page.items {
    println!("{}", task.get::<&str>("title")?.unwrap_or(""));
}
# Ok(())
# }
```

## subscribe

```rust no_run
# use futures_util::StreamExt;
# use nocentraldb::{Db, OpenOptions, Page};
# fn render(_: Page) {}
# #[tokio::main(flavor = "current_thread")]
# async fn main() -> Result<(), Box<dyn std::error::Error>> {
# let lock = std::fs::read_to_string("model.lock.json")?;
# let db = Db::open(OpenOptions::new(&lock).path("data")).await?;
let query = db.index("tasks", "byStatusPriority")?.where_eq(["open"]).take(50);

let mut pages = db.subscribe(query);
while let Some(page) = pages.next().await {
    render(page?);
}
# Ok(())
# }
```
