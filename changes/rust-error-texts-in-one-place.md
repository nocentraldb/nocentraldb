---
type: internal
tickets: [NOC-329]
components: [sdk-rust]
---
Every error message of the Rust SDK, and every piece of one, now lives in `texts/texts.json` with when it shows, and the gate fails on an error text written in the SDK's code. The messages themselves are unchanged.
