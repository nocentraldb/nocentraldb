# 2026-11-27 · NOC-578 · A first document's Markdown reaches the core as written, without the NFC normalisation of a string, in every SDK

- Decided by: coding agent (NOC-578)
- Why: the core reads the Markdown into the document, and the Rust SDK's `RichText::from_markdown` sends it as written; normalising it in TypeScript only would give the same Markdown a different document there, which the shared table (`from-markdown.json`) exists to rule out. NFC is RFC 0005's rule for `nc.string`, and a rich-text field is no string field. The TypeScript SDK sends it to the bridge as `{ $markdown: text }`.
- Instead of: NFC in the TypeScript SDK, as for a string, which would make the two SDKs differ; NFC in the core for both, which would change what Rust writes today.
- Topics: rich-text, rfc-0013, sdk-ts, sdk-rust
