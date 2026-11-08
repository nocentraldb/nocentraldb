# 2026-11-08 · NOC-19 · A panic in apply fails the test instead of becoming a reported C5 violation

- Decided by: coding agent (NOC-19)
- Why: on wasm32 a panic aborts and cannot be caught, and behaviour must not fork by platform (principle 7); a panic still fails the gate on both targets. The C5 fixture breaks convergence on malformed operations instead of panicking.
- Instead of: catching panics natively with `catch_unwind`, which would report C5 on one target only.
- Topics: convergence, testing, wasm
