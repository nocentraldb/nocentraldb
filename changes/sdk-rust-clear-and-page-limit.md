---
type: feature
tickets: [NOC-277, NOC-274]
components: [sdk-rust, core]
---
## Problem

The Rust SDK could not clear a field: a written text, number or link stayed until something else replaced it, though RFC 0009 lets `null` clear one in TypeScript. A page of a query was also capped at 500, while RFC 0009 now allows 1000 in both SDKs.

## Use

Clear a basic field or a link with `Value::clear()`; it reads unset afterwards. A create with a clear leaves the field unset. A clear of a personal field writes a plain null, so it needs no person and no person key. A clear of a set or of many links fails with `InvalidValue`:

```rust
project.update("tasks", task, [("title", Value::clear()), ("project", Value::clear())]).await?;
```

Take up to 1000 objects in one page; `take(0)` or more than 1000 fails with `InvalidValue`:

```rust
let page = db.find(db.index("tasks", "byPriority")?.take(1000)).await?;
```
