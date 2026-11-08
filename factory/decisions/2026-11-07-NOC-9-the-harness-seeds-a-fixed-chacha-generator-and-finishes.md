# 2026-11-07 · NOC-9 · The harness seeds a fixed ChaCha generator and finishes shrinking itself

- Decided by: coding agent (NOC-9)
- Why: without proptest's `std` feature, `Config::rng_seed` is ignored, so the harness builds a ChaCha `TestRng` from `Config::seed` (default 0) and every run of the gate is the same. Arrival keys travel with each change so that shrinking a change keeps the others' order, and a final pass removes single changes and duplicate deliveries that proptest's vector shrinker leaves behind; minimality is tested for seeds 0 to 63.
- Instead of: OS-random seeds per run (not deterministic, not on wasm32), or accepting counterexamples that are not minimal.
- Topics: convergence, testing
