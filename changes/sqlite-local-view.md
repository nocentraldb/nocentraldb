---
type: breaking
tickets: [NOC-69]
components: [core]
---
## Problem

`ncdb-core` folded received changes only into in-memory test doubles. A member had no local view on disk: no SQLite database holding its log and the objects and field values folded from it, as RFC 0006 defines.

## Use

Natively, `ncdb_core::view::View` is a member's database: the log of every space it belongs to and the view folded from it, in WAL mode with full durability. `View::receive` appends a frame, takes it through the checks and folds it, all in one transaction. `View::get` reads an object by id. `View::page` reads one declared index, page by page:

```rust
let mut view = View::open(path, model)?; // a view::Model until model.lock.json is read
view.receive(&frame, &mut checks)?;
let row = view.get(space, "task", object)?; // the object and its fields' merge states
let page = view.page("by_rank", &[], None, View::DEFAULT_PAGE)?;
let next = view.page("by_rank", &[], page.next.as_ref(), View::DEFAULT_PAGE)?;
```

The browser's view (OPFS) comes later; on wasm32 the module is not built.

## Migration

`receive::Fold::objects` is gone. Implement `Fold::fold_object(object, operation)` instead and apply the create or delete to your object, for example `self.objects.entry(object).or_default().apply(operation)`.
