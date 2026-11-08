# 2026-11-07 · NOC-9 · The harness lives in `crates/ncdb-core/tests/support/`, not in `src/`

- Decided by: coding agent (NOC-9)
- Why: the ticket keeps the `Mergeable` trait internal to the crate's test support; integration tests reach it with `mod support;`, and nothing of it enters the public API or cargo-semver-checks.
- Instead of: a `test-support` cargo feature that would expose the trait from `src/`.
- Topics: convergence, testing
