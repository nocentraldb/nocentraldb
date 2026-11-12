---
type: internal
tickets: [NOC-175]
components: [sdk-rust]
---
On wasm32 the Rust SDK's sync clock waits until a deadline has passed by `performance.now()`, so a sleep or a timeout no longer ends a little early when a `setTimeout` timer fires before its delay.
