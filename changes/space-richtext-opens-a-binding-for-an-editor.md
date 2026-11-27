---
type: feature
tickets: [NOC-461]
components: [sdk-ts]
---
## Problem
An app could not let a person edit a rich-text field (`nc.richText`) in an editor: nothing bound an editor's document to the field.

## Use
`space.richText(collection, id, field)` opens a binding: a `Y.Doc` that an editor such as TipTap edits, which the SDK writes as one change at each pause in typing and keeps in step with what other members write. `body.status` and `body.onStatus` say whether the editor shows the whole field and may change it. No release holds rich text in its core yet, so the call answers `notSupported` until one does; `yjs` is an optional peer dependency that only an app using rich text installs.

```ts
const body = await space.richText(model.notes, noteId, "body");
body.onStatus((status) => console.log(status.status));
// later
await body.close();
```
