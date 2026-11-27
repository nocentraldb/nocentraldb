---
type: feature
tickets: [NOC-568]
components: [sdk-rust]
---
## Problem

A reference between two objects moved together kept naming the old space. A member that holds only the new space read it as not accessible, where a link between the same objects reads ok.

## Use

`Space::move_objects` writes each reference among the moved objects with the new space, so it stays a reference into its own space. A reference to an object left behind keeps naming the old space and is not cleared, unlike a link, which is still cleared and counted in `cleared_links`.

```rust
space.move_objects(&[project, task], &shared).await?;
// task.customer now names `shared`, where the project moved too.
```
