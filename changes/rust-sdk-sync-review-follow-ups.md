---
type: internal
tickets: [NOC-134]
components: [sdk-rust]
---
The Rust SDK, not yet released, ignores live entries of a space it did not subscribe to in the current session, instead of folding them and syncing that space.
