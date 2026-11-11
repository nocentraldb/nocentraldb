---
type: feature
tickets: [NOC-216]
components: [sdk-rust]
---
## Problem
A member could not leave a space; only an admin could remove another member.

## Use
`space.leave()` removes this member from the space and returns a `Notice` with the code `NoticeCode::SpaceLeft`.

```rust
let notice = space.leave().await?;   // NoticeCode::SpaceLeft
```

The member keeps its copy of everything it received before. It does not rotate the space key, since it must not know the next one; until an admin rotates, the others write under a key it still holds, and `space.status()` reports the space as `exposed`. The space's last admin cannot leave (`ErrorCode::LastAdmin`).
