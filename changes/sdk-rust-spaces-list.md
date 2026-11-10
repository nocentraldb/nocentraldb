---
type: internal
tickets: [NOC-151]
components: [sdk-rust]
---
The Rust SDK's `spaces().list()` returns a `SpaceInfo { id, role, model }` for every space the copy holds, with the member's `Role` from the folded membership (`None` once removed) and `SpaceModel::Ours` or `SpaceModel::Another`, as RFC 0009 describes.
