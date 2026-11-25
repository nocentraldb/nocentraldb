---
type: breaking
tickets: [NOC-452]
components: [core]
---

## Problem

A log in `ncdb-core` kept every change in memory, so a long-lived space held more memory than a phone or a browser tab can give it. A view's log now releases a sealed change's bytes once its `log` table holds the frame (NOC-452). A call that reads changes must then say where a released change is read from, or it would hand out fewer changes than the log holds without saying so.

## Use

The calls that read a log's changes take the store of its frames, a `log::Frames`: `View::stored_frames()` for a view's log, `log::NoFrames` for a log that never releases a change, such as a test's. A `Fold` is the frames of the store it folds into.

```rust
let mut frames = view.stored_frames();
for (position, change) in view.log(space).unwrap().after(Position::START, &mut frames) {
    // `change` is a Cow<Change>: from memory, or read from the view's database.
}
frames.finish()?;
```

## Migration

- `Log::after(cursor)` becomes `Log::after(cursor, &mut frames)` and yields `(Position, Cow<Change>)` instead of `(Position, &Change)`.
- `Log::tips(checks)` becomes `Log::tips(checks, &mut frames)`, and `Log::newest_model(checks, model)` becomes `Log::newest_model(checks, model, &mut frames)`.
- `moves::later_edits(log, checks)` and `moves::moved_objects(log, checks)` take `&mut frames` as their third argument.
- `Log::get(position)` gives a change only while the log holds it in memory; `Log::change_at(position, &mut frames)` and `Log::change(id, &mut frames)` read a released one. `Log::controls()` gives the control changes, which a log always holds.
- Pass `&mut log::NoFrames` for a log you built yourself and never released a change of, and `&mut view.stored_frames()` for a view's log.
- Only code that uses `ncdb-core` directly is affected; the SDKs' APIs do not change.
