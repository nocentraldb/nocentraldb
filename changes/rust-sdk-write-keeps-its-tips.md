---
type: internal
tickets: [NOC-138]
components: [core, sdk-rust]
---
A write in the Rust SDK takes its heads, sequence and clock from the tips the core's log keeps as it folds, not from a pass over the whole log.
