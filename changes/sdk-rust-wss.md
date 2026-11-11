---
type: feature
tickets: [NOC-121]
components: [sdk-rust]
---
## Problem
The Rust SDK refused every relay URL that was not `ws://`, so it could not sync with a relay on the network, which RFC 0007 requires to speak TLS.

## Use
Open the database with a `wss://` relay URL; the SDK verifies the relay's certificate against the platform's trust store, and fetches large changes over `https://` the same way.

```rust
let db = Db::open(OpenOptions::new(LOCK).path("data").relay("wss://relay.example.com")).await?;
```
