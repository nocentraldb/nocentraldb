# 2026-11-08 · NOC-16 · A harness generator for C3 draws no `usize`

- Decided by: coding agent (NOC-16)
- Why: proptest draws a `usize` differently on 32-bit wasm32 than on 64-bit native targets, so the two runs generated different cases and the C3 digest differed although the type was right. The basic values adapter draws `u8`; NOC-17's adapter must avoid `usize` too.
- Instead of: committing two digests, one per target, which would hide a real platform difference.
- Topics: testing, convergence, wasm
