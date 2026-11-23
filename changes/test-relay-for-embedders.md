---
type: feature
tickets: [NOC-507]
components: [relay, sdk-rust]
---
## Problem
An app or engine that embeds the Rust SDK could test its sync only against a mock: the relay the SDK's own tests start was not public, and no relay crate was on crates.io.

## Use
Add `ncdb-relay-server` to your dev-dependencies and start a test relay on SQLite in memory and a free port. It stops when dropped:

```rust
use ncdb_relay_server::local::{Options, TestRelay};

let relay = TestRelay::start(Options::new().admit_any())?;
let options = OpenOptions::new(LOCK)
    .in_memory()
    .relay(&relay.url())
    .admission(relay.admissions());
let db = Db::open(options).await?;
```

`admit_any` lets any account create spaces through admissions the test relay signs; `allow_identity` lists one account by the identity `db.account().identity()` reports instead. With neither, the relay refuses a new space with `not_allowed`. `admission_key` and `relay_id()` test admissions. `ncdb-relay` and `ncdb-relay-server` go to crates.io with the next beta.
