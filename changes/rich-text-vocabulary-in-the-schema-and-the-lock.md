---
type: breaking
tickets: [NOC-554]
components: [core, sdk-ts, cli]
---
## Problem

The editor's extensions alone decided what a rich-text field may hold: adding an extension silently widened the field, and nothing in the data model recorded it.

## Use

A rich-text field declares its vocabulary, the names of the nodes and marks its editor uses:

```ts
export const model = nc.model({
  notes: nc.collection({
    body: nc.richText({
      vocabulary: { nodes: ["paragraph", "heading", "callout"], marks: ["bold", "link"] },
    }),
  }),
});
```

`ncdb model build` writes it into the lock under the field's key `vocabulary`, each list sorted and without duplicates, and lists added names. A later model may add names; removing one is refused by `ncdb model build`, `ncdb model check` and the check when a database opens. `nc.open` refuses a lock whose vocabulary differs from the model's, as for any field changed without a build. `nc.richText()` without the option declares an empty vocabulary and builds the same lock as before.

## Migration

Apps and their locks need no change.

Rust code that builds an `ncdb_core::model_lock::Field` with a struct literal adds the new field `vocabulary` (`Vocabulary::default()` for any field that is not rich text), and an exhaustive `match` on `Refusal` handles the new variants `NodeRemoved` and `MarkRemoved`.
