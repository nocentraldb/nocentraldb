# 2026-11-07 · NOC-18 · check-pins leaves path dependencies to check-version

- Decided by: coding agent (NOC-18)
- Why: `scripts/release/check-version.mjs` already owns the rule for dependencies between workspace crates (declared in `[workspace.dependencies]` with `version = "=<workspace version>"`, never `path =` in a crate); one rule, one owner, one message.
- Instead of: check-pins checking `path =` entries a second time and reporting the same mistake twice.
- Topics: dependencies, gates, release
