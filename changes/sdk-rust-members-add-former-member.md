---
type: internal
tickets: [NOC-169]
components: [sdk-rust]
---
The Rust SDK's `members().add` adds a former member again: it rotates the space key to a fresh generation first, with an envelope for every member, then adds the member from that generation (RFC 0008, M5), instead of refusing it with `InvalidValue`.
