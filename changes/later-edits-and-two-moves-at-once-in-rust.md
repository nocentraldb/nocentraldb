---
type: feature
tickets: [NOC-475]
components: [sdk-rust]
---
## Problem
After a move, edits that other members made to the object in the old space without having seen the move stayed there, and an app could not find them, or tell that two members had moved one object to two spaces at once (RFC 0016).

## Use
`space.moves().later_edits()` lists each moved object's later edits, the same on every member and after a reopen; `carry_over(&edits)` writes chosen ones into the new space as one write; `conflicts()` names the objects moved to two spaces at once, the space that won and the ones that lost.

```rust
for moved in personal.moves().later_edits().await? {
    personal.moves().carry_over(&moved.edits).await?;
}
```
