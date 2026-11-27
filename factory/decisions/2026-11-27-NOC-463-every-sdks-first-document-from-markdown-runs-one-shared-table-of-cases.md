# 2026-11-27 · NOC-463 · Every SDK's first document from Markdown runs one shared table of cases, `crates/ncdb-core/tests/vectors/rich-text/from-markdown.json`

- Decided by: coding agent (NOC-463)
- Why: the core reads the Markdown back for every SDK, so the same Markdown must give the same document in Rust and TypeScript (NOC-578). The table holds, for each case, the Markdown, the document as the tree the core reads (ProseMirror JSON under a `doc` root) and the Markdown the field then reads as; and, under `refused`, Markdown the core does not write a document from. The core's test, the Rust SDK's test of `RichText::from_markdown` and the TypeScript SDK's test of `nc.richText.fromMarkdown` each run every case, so a case added once binds all three.
- Instead of: a table per SDK, which can drift apart; or comparing the SDKs' outputs with each other only, which pins no expected document.
- Topics: rich-text, rfc-0013, sdk-rust, sdk-ts, tests
