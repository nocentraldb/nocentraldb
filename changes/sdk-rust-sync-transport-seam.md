---
type: internal
tickets: [NOC-175]
components: [sdk-rust]
---
The Rust SDK's sync session talks to the relay through a transport seam and waits on a clock of its own, so the browser's WebSocket can carry it on wasm32; nothing changes natively.
