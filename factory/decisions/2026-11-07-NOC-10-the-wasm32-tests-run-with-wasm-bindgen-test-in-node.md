# 2026-11-07 · NOC-10 · The wasm32 tests run with wasm-bindgen-test in Node

- Decided by: coding agent (NOC-10)
- Why: `wasm-bindgen-test` is the standard runner for `wasm32-unknown-unknown`, the browser target, and Node needs nothing the gate does not already have; CI runs it through `pnpm gates` without a workflow change. The runner (`wasm-bindgen-cli =0.2.130`, the `wasm-bindgen` that Cargo.lock holds) is built by `cargo install --locked` into `target/tools`, which the existing Cargo cache keeps; a cold build takes about a minute.
- Instead of: a headless Chromium run (needs a browser in CI for no extra coverage of the core today), a `wasm32-wasip1` target (it has `std::fs`, so it would hide exactly what the gate must catch), or downloading a prebuilt runner binary (a second pinning and checksum path).
- Topics: wasm, testing, ci
