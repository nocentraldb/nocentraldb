# 2026-11-07 · NOC-14 · The change and log implementation stacks on NOC-10 and NOC-9

- Decided by: pm
- Why: all three change `crates/ncdb-core`, and new test files need NOC-10's wasm32 test routing; stacking avoids merge conflicts in Cargo.toml and Cargo.lock while #29 and #34 wait for the owner.
- Instead of: branching from main and resolving conflicts after the owner merges #29 and #34.
- Topics: planning, factory
