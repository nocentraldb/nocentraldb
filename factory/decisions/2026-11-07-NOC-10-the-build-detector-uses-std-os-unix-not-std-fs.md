# 2026-11-07 · NOC-10 · The build detector uses std::os::unix, not std::fs

- Decided by: coding agent (NOC-10)
- Why: `std::fs` compiles for `wasm32-unknown-unknown` and fails only when called, so it cannot prove the build task fails; the fixture uses `std::os::unix`, which does not exist there. A call into `std::fs` at runtime is caught by the test-wasm task instead, as long as a test reaches it.
- Instead of: the ticket's example `std::fs`, which would have made the detector test pass the build and prove nothing.
- Topics: wasm, testing
