# 2026-11-08 · NOC-19 · C3 is a committed SHA-256 over the final state bytes, with sha2

- Decided by: coding agent (NOC-19)
- Why: the same test runs natively and in wasm32 (NOC-10) and compares with one committed value, so neither run needs the other's output. `sha2 =0.11.1` is the audited crate and version PR #41 pins for change ids; the harness uses that same dependency, so it adds nothing to the shipped crate.
- Instead of: hashing with a hand-written function (GENESIS.md: never invent cryptography), or comparing the two runs' outputs in a separate script.
- Topics: convergence, testing, crypto
