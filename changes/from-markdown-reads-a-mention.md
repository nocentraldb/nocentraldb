---
type: feature
tickets: [NOC-536]
components: [core, sdk-rust, sdk-ts]
---
## Problem
An app that imports text with mentions builds Markdown and writes it with `nc.richText.fromMarkdown` or `RichText::from_markdown`. A mention in that Markdown, its link form `[@](ncdb:<space id>/<object id>)`, had no promise to come back as a mention, and a malformed `ncdb:` link was written as a plain link.

## Use
Write each mention in its link form, with no name:

```ts
await space.create(model.notes, {
  body: nc.richText.fromMarkdown(`Ask [@](ncdb:${person.$space}/${person.$id}) about it.`),
});
```

The field reads back as the same Markdown, and the Rust SDK's `mentions()` lists the reference. Any other `ncdb:` link, such as one without both ids or with a name as its text, is refused with `invalidValue` and nothing is written.
