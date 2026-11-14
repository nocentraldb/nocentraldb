# 2026-11-14 · NOC-178 · The end-to-end test of a browser tab and a Rust device runs in the browser CI job, not in gates

- Decided by: pm, NOC-178 (A: runs in the browser CI job like its siblings, not in gates, following moon.yml's rule)
- Why: the ticket asks for the test "in `pnpm gates`", but moon.yml keeps every Playwright test out of `gates`, because they need installed browsers. `scripts/sdk-browser/rust.e2e.test.mjs` is a new file of the existing task `repo:sdk-browser`, which the workflow browser-view.yml runs. The workflow builds its Rust device, the example `crates/nocentraldb/examples/browser_peer.rs`, on the runner, as it builds the relay, and runs it in Playwright's image, which has no Rust. The test takes about 2 s on the built packages and about 75 s with the builds, inside the ticket's two minutes.
- Instead of: adding the test to `gates`, which would make every run of `pnpm gates` and the gates CI job need Chromium; or a Rust test file the browser test drives, which would mix the test harness's output into the protocol.
- Topics: tests, browser, sdk, relay
