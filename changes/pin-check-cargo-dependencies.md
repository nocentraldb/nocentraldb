---
type: internal
tickets: [NOC-18]
components: [core]
---
The pin check `repo:pins` now fails on any Cargo dependency that is not pinned to an exact `=x.y.z`, and refuses git dependencies and `[patch]`/`[replace]` sections.
