# 2026-11-20 · NOC-371 · The landing page's TypeScript code lives in code-ts.md, one module per step, its hidden lines marked "# " as rustdoc's

- Decided by: pm
- Why: the gates type-check each step against the SDK as the doc tests compile code-rust.md, and the setup a snippet needs stays off the page, written the same way in both files
- Instead of: the snippets as strings in CodeSection.astro with the setup supplied by the checker, which keeps what the page shows apart from what is checked
- Topics: site, landing-page, docs, sdk-ts
