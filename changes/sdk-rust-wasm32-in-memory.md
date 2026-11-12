---
type: internal
tickets: [NOC-175]
components: [sdk-rust]
---
The Rust SDK builds for wasm32-unknown-unknown and runs in memory there, with its in-memory tests in the gates; syncing with a relay is refused on wasm32 until the browser's WebSocket is built.
