---
type: internal
tickets: [NOC-578, NOC-589]
components: [sdk-ts]
---
The TypeScript SDK reads a rich-text field as its Markdown in `space.get` and queries, and writes a first document with `nc.richText.fromMarkdown` through the core's Markdown read-back, behind the compiler cfg `ncdb_rich_text` (RFC 0013).
