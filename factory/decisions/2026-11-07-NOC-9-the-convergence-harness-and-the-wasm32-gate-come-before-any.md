# 2026-11-07 · NOC-9, NOC-10 · The convergence harness and the wasm32 gate come before any field type

- Decided by: pm
- Why: they are the only core work that needs no owner decision, and every later field type is then built test first against a fixed definition of "converges" on both targets (principles 2 and 7).
- Instead of: starting with field types and adding the harness and the wasm build afterwards.
- Topics: planning, convergence, wasm
