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

let relay = TestRelay::start(Options::new().allow_identity(&identity))?;
let db = Db::open(OpenOptions::new(LOCK).path(dir).key_file(keys).relay(&relay.url())).await?;
```

`allow_identity` takes the identity `db.account().identity()` reports; without it, the relay refuses a new space with `not_allowed`. `admission_key` and `relay_id()` test admissions. `ncdb-relay` and `ncdb-relay-server` go to crates.io with the next beta.
