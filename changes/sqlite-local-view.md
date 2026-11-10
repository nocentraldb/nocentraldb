---
type: breaking
tickets: [NOC-69, NOC-77, NOC-81]
components: [core]
---
## Problem

`ncdb-core` folded received changes only into in-memory test doubles. A member had no local view on disk: no SQLite database holding its log and the objects and field values folded from it, as RFC 0006 defines.

## Use

Natively, `ncdb_core::view::View` is a member's database, folded by the model of its `model.lock.json`: the log of every space it belongs to and the view folded from it, in WAL mode with full durability. `View::receive` appends a frame, takes it through the checks and folds it, all in one transaction. `View::get` reads an object by id, deleted or not. `View::page` reads one declared index of a collection, page by page, and `View::page_in` reads an index declared with `scope: "space"` in one space. No index lists a deleted object:

```rust
let lock = ModelLock::from_json(&std::fs::read_to_string("model.lock.json")?)?;
let mut view = View::open(path, &lock)?;
view.receive(&frame, &mut checks)?;
let row = view.get(space, "task", object)?; // the object and its fields' merge states
let page = view.page("task", "byRank", &[], None, View::DEFAULT_PAGE)?;
let next = view.page("task", "byRank", &[], page.next.as_ref(), View::DEFAULT_PAGE)?;
let mine = view.page_in(space, "task", "inSpace", &[], None, View::DEFAULT_PAGE)?;
```

The browser's view (OPFS) comes later; on wasm32 the module is not built.

## Migration

`receive::Fold::objects` is gone. Implement `Fold::fold_object(object, operation)` instead and apply the create or delete to your object, for example `self.objects.entry(object).or_default().apply(operation)`.
