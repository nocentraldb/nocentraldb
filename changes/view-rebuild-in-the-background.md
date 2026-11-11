---
type: feature
tickets: [NOC-156]
components: [core, sdk-rust]
---
## Problem
When a database had to fold its whole log again, as after an app was rolled back to an older model, `Db::open` returned only once that was done. On a large database the app waited at start, and it could not show why.

## Use
`Db::open` now returns at once. The app reads the old view meanwhile, and the SDK rebuilds the view in the background, then swaps the new one in. `db.view()` reports whether a rebuild runs and how far it is, in frames of the log.

```rust
let mut view = db.view().subscribe();
while let Some(status) = view.next().await {
    match status.rebuild {
        Some(progress) => show_progress(progress.folded, progress.total),
        None => break,
    }
}
```
