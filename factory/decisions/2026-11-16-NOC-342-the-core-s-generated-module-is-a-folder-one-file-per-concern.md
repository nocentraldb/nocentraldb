# 2026-11-16 · NOC-342 · The core's generated module is a folder, one file per concern

- Decided by: coding agent, NOC-342 (NOC-329 T6), within pm's decision that `scripts/texts.mjs --write` generates the core's module
- Why: pm's decision named the module `crates/ncdb-core/src/texts.rs`. With all 121 core texts it held 827 lines, over the repository's 700-line limit for a file. So it is a folder, `crates/ncdb-core/src/texts/`, as T2 made the SDK's.
  - **One file per concern.** `account`, `change`, `lock`, `value` and `view`. Each file holds the texts whose name begins with one of its concern's words, listed in `CONCERNS` in `scripts/texts-core.mjs`; for example, `view` holds `core.view*` and `core.unreadable*`.
  - **The gate checks the names.** A core text whose name begins with no concern's word fails it, so a new text can't end up in a file by accident.
  - **Call sites don't change.** `mod.rs` re-exports every file, so code still names a text `texts::NAME`.
- Instead of: one file per kind of error, which would make 17 files, many of one or two texts; or naming the file in each entry, which would add a field to texts.json that only the core's texts need.
- Topics: texts, core, gates
