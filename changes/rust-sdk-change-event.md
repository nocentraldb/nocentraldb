---
type: feature
tickets: [NOC-506]
components: [sdk-rust]
---
## Problem
The Rust SDK had no change event. An embedder that reacts to changes, such as an agent's wake-up logic, could only follow whole query pages, and could not tell which objects a fold touched or whether the change was its own write.

## Use
`db.changes().subscribe()` yields one `Change` per collection each fold touched, as `db.changes.subscribe` does in TypeScript. It carries ids, never values.

```rust
let mut changes = db.changes().subscribe();
while let Some(change) = changes.next().await {
    // change.space, change.collection, change.ids
    if change.origin == Origin::Remote { /* it came by sync */ }
}
```
