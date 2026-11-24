---
type: feature
tickets: [NOC-475]
components: [sdk-rust, core]
---
## Problem
A Rust app could not move an object to another space, for example to share a single note: it had to copy the note into a new space and delete it, which gave it a new id, broke its links and left offline edits on the deleted original (RFC 0016).

## Use
`space.move_objects(&objects, &destination)` moves objects read from `space`, with their ids and values, and returns `Moved { notice, ids, cleared_links, conflicts }`. The old space keeps a trace: `object.moved_to()` names the new space, the trace shows no fields, and a write to it fails with `ErrorCode::Moved`. Objects with a personal value or a set plugin field cannot be moved yet (`NotAllowed`).

```rust
let shared = db.spaces().create().await?;
let note = personal.get("notes", id).await?.unwrap();
let moved = personal.move_objects(&[note], &shared).await?;
assert_eq!(moved.notice.code, NoticeCode::ObjectMoved);
```
