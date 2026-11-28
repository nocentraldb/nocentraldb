---
type: internal
tickets: [NOC-463]
components: [core, sdk-rust]
---
The Rust SDK reads a rich-text field with `get::<RichText>` as Markdown, plain text, tree and mentions, and writes a first document with `RichText::from_markdown` through the core's Markdown read-back, behind the compiler cfg `ncdb_rich_text` (RFC 0013, step 6).
