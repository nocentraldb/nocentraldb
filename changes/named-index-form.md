---
type: feature
tickets: [NOC-563]
components: [sdk-ts, sdk-rust]
---
## Problem
An index spelled its equality prefix as a count: `nc.index(["status", "priority"], { equal: 1 })` said which fields a query fixes only by counting them, and a descending field took an object, `{ field, direction: "desc" }`.

## Use
Name the fields a query fixes in `where`, and the fields that order the result in `orderBy`, with a leading `-` for a descending one. A query fixes the `where` fields with `.where(...)` in TypeScript and `.where_eq(...)` in Rust; `null` in TypeScript and `Value::clear()` in Rust fix a field to unset, in `.equal(...)` too.

```ts
indexes: {
  byStatusPriority: nc.index({ where: ["status"], orderBy: ["priority"] }),
  latest: nc.index({ orderBy: ["-occurredOn"] }),
}

const page = await db.find(model.tasks.indexes.byStatusPriority.where("open").take(50));
```

```rust
let query = db.index("tasks", "byStatusPriority")?.where_eq(["open"]).take(50);
```

The named form builds the same `model.lock.json` as `nc.index(fields, { equal })`, byte for byte, so switching an index to it changes no model hash and refuses no cursor. The older form and `.equal(...)` stay valid. `ncdb model build` refuses a named index that names no field, names a field twice, or names a field in both `where` and `orderBy`.
