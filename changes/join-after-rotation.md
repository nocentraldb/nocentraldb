---
type: internal
tickets: [NOC-139]
components: [core, sdk-rust]
---
`ncdb-core`'s checks open the previous keys of the key chain back to the first generation, so in the Rust SDK a member added after one or more key rotations opens its space with `spaces().get` and reads the changes sealed before it joined.
