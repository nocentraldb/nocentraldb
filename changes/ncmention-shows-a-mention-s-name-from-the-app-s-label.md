---
type: feature
tickets: [NOC-461]
components: [sdk-ts]
---
## Problem
A rich-text field had no mention that kept a person's name out of the text: TipTap's own Mention extension stores the name as typed in the document, where an erasure does not reach it.

## Use
`NcMention` from `@nocentraldb/tiptap` is an inline node `mention` that stores only the target's `space` and `id`. The editor shows the name the app's `label` gives for the target, a string or a promise of one, and a neutral placeholder until then and whenever `label` gives none, as after an erasure. Opening or rebuilding an editor with `ncEditor` no longer writes what plugins change as the document renders, such as a link autolink makes of URL-like text. The package is not published yet.

```ts
const mounted = ncEditor(body, {
  element: document.querySelector("#body"),
  extensions: [
    StarterKit.configure({ undoRedo: false }),
    NcMention.configure({ label: ({ space, id }) => personName(space, id) }),
  ],
  onEditor: (next) => {
    editor = next;
  },
});
editor.commands.insertContent({ type: "mention", attrs: { space, id } });
```
