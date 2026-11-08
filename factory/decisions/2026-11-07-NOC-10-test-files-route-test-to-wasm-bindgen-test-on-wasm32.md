# 2026-11-07 · NOC-10 · Test files route #[test] to wasm_bindgen_test on wasm32

- Decided by: coding agent (NOC-10)
- Why: wasm-bindgen-test skips plain `#[test]` and the runner still exits 0 ("no tests to run!"), so the gate would pass with no test run. Each test file imports `wasm_bindgen_test as test` under `cfg(target_arch = "wasm32")`, the tests stay unchanged natively, and `scripts/wasm-gate.test.mjs` fails when a file of ncdb-core has a `#[test]` without that import.
- Instead of: writing `#[wasm_bindgen_test]` next to every `#[test]`, which a new test can forget without any check noticing.
- Topics: wasm, testing
