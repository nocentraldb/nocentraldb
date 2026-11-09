# 2026-11-09 · NOC-70 · ncdb runs the TypeScript schema with Node.js, and the core builds and checks the lock

- Decided by: coding agent (NOC-70), agreed by pm; the owner reviews the PR because RFC 0005 does not name Node.js
- Why: release-management.md publishes the CLI as the crate `ncdb` (`cargo install ncdb`), so it lives in `crates/ncdb`, binary `ncdb`. The schema is a TypeScript module, so something must run it. `ncdb model build` runs `node` (24 or later strips TypeScript types natively) on a short script that imports the schema and prints the description `nc.model` keeps, and does everything else in Rust. Building the lock and the additive-change rule live in `ncdb_core::model_lock`, so the device check of RFC 0005 D11 can run the same rule natively and in wasm. `ncdb model build` and `check` therefore need Node.js on PATH; the CLI refuses a missing `node`, or one older than 24, with a message that names what it found.
- Instead of: a CLI written in TypeScript, which the release docs' crate rules out, or the rule written twice, once in TypeScript for the build and once in Rust for the device.
- Topics: cli, schema, model-lock, rfc-0005, node
