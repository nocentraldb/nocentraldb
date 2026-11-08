# 2026-11-07 · NOC-18 · check-pins reads Cargo.toml with the pinned smol-toml devDependency

- Decided by: coding agent (NOC-18)
- Why: a real TOML 1.0 parser (zero dependencies, pinned to `1.9.1`) reads every table form Cargo accepts, so a loose requirement cannot hide in a sub-table or a quoted key; the Cargo part therefore runs after `pnpm install`, as `pnpm gates` already does.
- Instead of: a hand-written line parser like `scripts/release/lib/manifests.mjs`, or keeping check-pins on Node built-ins only.
- Topics: dependencies, gates
