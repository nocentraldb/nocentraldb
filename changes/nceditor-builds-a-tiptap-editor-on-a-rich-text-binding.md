---
type: feature
tickets: [NOC-461]
components: [sdk-ts]
---
## Problem
A TipTap editor on a rich-text binding deleted what its schema lacked, such as a node a newer version of the app added, went stale after a rebuild, and could be built without TipTap's content check.

## Use
`ncEditor(body, { element, extensions, onEditor })` from `@nocentraldb/tiptap` builds the editor on a binding from `space.richText`: it checks the extensions against the field's vocabulary, sets the content check, widens the schema with placeholders for what it does not know, rebuilds when a new kind of node arrives or the binding reloads, and hands each editor to `onEditor`. It tells the binding its placeholders through `body.known`, so a person may delete one on purpose. An app that builds its own editor adds `NcKeepUnknown`, which refuses an editor without the content check with `contentCheckMissing`. The package is not published yet.

```ts
const body = await space.richText(model.notes, noteId, "body");
let editor;
const mounted = ncEditor(body, {
  element: document.querySelector("#body"),
  extensions: [StarterKit.configure({ undoRedo: false })],
  onEditor: (next) => {
    editor = next;
  },
});
// later
mounted.destroy();
await body.close();
```
