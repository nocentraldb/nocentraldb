---
type: feature
tickets: [NOC-256]
components: [sdk-rust]
---
## Problem
An app learned that another device had shared a space with this member, or that a space was left, removed or changed role, only by polling `db.spaces().list()`.

## Use
`db.spaces().subscribe()` returns a stream of `Vec<SpaceInfo>`: the current list at once, then each new list when a space is added, left or removed, or the member's role in one changes. A list that did not change is not yielded again; dropping the stream stops it.
