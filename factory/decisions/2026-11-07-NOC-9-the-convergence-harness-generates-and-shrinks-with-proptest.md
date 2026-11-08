# 2026-11-07 · NOC-9 · The convergence harness generates and shrinks with proptest

- Decided by: coding agent (NOC-9)
- Why: proptest shrinks through arbitrary strategy compositions, so a divergence shrinks without hand-written shrinkers, and its runner takes an explicit `TestRng`; without default features (`alloc`, `no_std`) it pulls no threads, no fork and no OS randomness (no getrandom), so the harness stays usable on wasm32 (NOC-10). Pinned to `=1.12.0` as a dev-dependency only.
- Instead of: quickcheck, whose shrinking does not compose through mapped generators, or a hand-rolled generator and shrinker.
- Topics: convergence, testing, dependencies
