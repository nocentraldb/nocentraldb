---
type: breaking
tickets: [NOC-99]
components: [sdk-ts, cli]
---
## Problem
A schema could not declare a reference to an object in another space, so `ncdb model build` never wrote the field type `ncdb/ref` that the core reads from `model.lock.json`.

## Use
Declare a reference with `nc.ref("collection")`, or with a list of collections, as `nc.link` takes them. It takes `.describe(...)` and `.deprecated(...)`, and cannot be personal. `nc.Infer` types it as `{ space, id }`, the target's space id and object id.

```ts
orders: nc.collection({
  customer: nc.ref("customers"),
  buyer: nc.ref(["customers", "leads"]).describe("Who pays."),
}),
```

`ncdb model build` writes `{ "type": "ncdb/ref", "rule": 1, "targets": [...] }` and lists the new field as `+ orders.customer ncdb/ref -> customers`. Adding a collection to the targets is accepted and listed as `+ orders.customer targets leads`; removing one is refused, as for links.

## Migration
No code needs to change: every schema that compiled before still compiles and builds the same lock. The API report lists the change as breaking because the internal types every field shares (`Field`, `Fields`, and the condition `nc.Infer` uses to leave out deprecated fields) were reworked so that a field without `.personal` fits in a collection.
