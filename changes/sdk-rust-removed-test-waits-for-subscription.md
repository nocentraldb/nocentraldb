---
type: internal
tickets: [NOC-184]
components: [sdk-rust]
---
The Rust SDK's removal test waits until the member's subscription caught up, instead of racing the relay's catch-up.
