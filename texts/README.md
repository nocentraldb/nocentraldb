# Texts

Every text a developer reads from the SDKs, the `ncdb` command line and the core lives here, one file per text (NOC-329, NOC-512): `texts/<kind>/<name>.json`, for the text whose id is `<kind>.<name>`. Two pull requests that each add a text add two files, so they do not conflict.

Each file holds one entry, written as `JSON.stringify(entry, null, 2)` writes it, one field per line:

- `id`: `<kind>.<name>`, with a camelCase name; it matches the file's path.
- `kind`: `notice`, `error`, `part` (a piece of an error's message, such as a reason), `cli` (a text of the command line), `ts` (a text of the TypeScript SDK) or `core` (a text the core words itself).
- `code`: for a notice or an error, its code as RFC 0009 spells it; an error returned with several codes names each, as in `storageFull | storageFailed`. A `ts` text carries one when the SDK throws it as an `nc.Error`. A `part`, `cli` or `core` text has none.
- `text`: the text, with `{name}` or `{name:?}` where a value goes, and a literal brace doubled, `{{` or `}}`, as Rust writes it.
- `when`: when it shows.

After adding or changing a text, run `node scripts/texts.mjs --write`. It regenerates `crates/nocentraldb/src/texts/`, `crates/ncdb/src/texts.rs`, `packages/sdk/src/texts.ts` and `crates/ncdb-core/src/texts/`, which list the texts by a hash of their ids, so that two new texts rarely land side by side. The SDK's error texts stand in `crates/nocentraldb/src/texts/error/`, in four files by a hash of their ids, which `error/mod.rs` re-exports, so code names them `texts::error::NAME`. The gate `repo:texts` (`node scripts/texts.mjs --check`) checks the files and the generated modules, and fails when a generated file would pass the limit for a file in `factory/CONFIG.md`; then raise the kind's number in `SHARDS` in `scripts/texts.mjs`, or, for the core, add a concern to `CONCERNS` in `scripts/texts-core.mjs`.

`pnpm texts` lists every text new, changed or removed since the last release, for the owner's review before a beta or a stable release ([docs/release-management.md](../docs/release-management.md)).
