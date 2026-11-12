---
type: internal
tickets: [NOC-175, NOC-231]
components: [sdk-rust, core]
---
On wasm32 the Rust SDK opens a database at a name on OPFS in a dedicated worker, with its keys from IndexedDB, and keeps a crate-internal durability report; the core's log keeps when the member wrote each own change (`written_at`). Nothing public names the browser database yet.
