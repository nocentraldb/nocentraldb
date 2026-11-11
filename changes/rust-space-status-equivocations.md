---
type: feature
tickets: [NOC-190]
components: [core, sdk-rust]
---
## Problem
A Rust app could not tell that one member key writes from two places, as a device restored from a backup or cloned does: two different changes of one author under one sequence, which RFC 0002 calls an equivocation.

## Use
`space.status().current().equivocations` lists the changes of every equivocation in the space, in local order. Both changes of an equivocation stay folded; the list lets an app warn about the device that wrote them.

```rust
if !project.status().current().equivocations.is_empty() {
    show("A device of this space writes from two places");
}
```
