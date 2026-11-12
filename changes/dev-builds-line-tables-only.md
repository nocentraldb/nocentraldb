---
type: internal
tickets: [NOC-279]
components: [core, sdk-rust, cli, relay]
---
Debug and test builds keep only line tables (`debug = "line-tables-only"`), so a full `pnpm gates` needs about 15 GiB of `target/` instead of more than 27 GiB, and panics still name file and line; release builds are unchanged.
