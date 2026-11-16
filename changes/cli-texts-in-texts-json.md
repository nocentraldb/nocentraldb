---
type: internal
tickets: [NOC-329]
components: [cli]
---
The `ncdb` command line's texts move into `texts/texts.json`, with when each one shows: the usage, its errors and status lines, the schema script's two messages and the build report's sentences. They are kind `cli`, have no code, and generate `crates/ncdb/src/texts.rs`. The gate `repo:texts` now fails on a text written in the command line's code too. One text changes: a schema module that exports no model now reads "export the model as `export const model = nc.model(...)`", because a text may hold no brace but its placeholders.
