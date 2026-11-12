---
type: internal
tickets: [NOC-156]
components: [sdk-rust]
---
The Rust SDK's thread that runs a rebuild of the view is now compiled natively only, as `Db::open` already started it only natively. A wasm32 build no longer carries it as dead code, and `cargo clippy --target wasm32-unknown-unknown` passes with warnings denied. Nothing changes in behaviour.
