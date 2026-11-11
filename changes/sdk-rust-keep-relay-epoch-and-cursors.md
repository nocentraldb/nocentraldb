---
type: internal
tickets: [NOC-145]
components: [core, sdk-rust]
---
A reopened Rust database reads back the relay epoch from its view's `meta` table and derives each space's cursor from the stored relay positions, so it pulls from where it stopped instead of from 0; a different epoch in `welcome` still makes every position unconfirmed.
