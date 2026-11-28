---
type: breaking
tickets: [NOC-578, NOC-589]
components: [sdk-ts]
---
## Problem

In TypeScript, a rich-text field read as `null` even after content was written to it, and an app could not write a first document into one, although RFC 0013 says that a read gives the field's Markdown and that `nc.richText.fromMarkdown` writes a first document.

## Use

A read gives a rich-text field's Markdown, a string; a field nothing was written to reads `null`, as any unset field. `nc.richText.fromMarkdown` writes a first document into a field that holds no content, on `create` or `update`:

```ts
const { ids } = await space.create(model.notes, {
  title: "Plan",
  body: nc.richText.fromMarkdown("## Plan\n\nShip **it**."),
});
const note = await space.get(model.notes, ids[0]);
note?.body; // "## Plan\n\nShip **it**."
```

The core reads the Markdown into the document, as the Rust SDK's `RichText::from_markdown` does, so both SDKs write the same document from the same Markdown. Into a field that holds content the write is refused with `richTextNotEmpty`; Markdown the core does not write a document from, such as HTML or an image, with `invalidValue`. Rich text is behind the compiler cfg `ncdb_rich_text`: no release folds it yet.

## Migration

`nc.richText` is now a function with a `fromMarkdown` member; calling it declares a field as before. A create or an update now takes only `nc.richText.fromMarkdown(...)` for a rich-text field: code that wrote a string or `null` there no longer type-checks, which the core already refused at run time with `invalidValue`.
