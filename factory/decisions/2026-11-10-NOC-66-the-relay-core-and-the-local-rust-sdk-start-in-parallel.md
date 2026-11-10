# 2026-11-10 · NOC-66 · The relay core and the local Rust SDK start in parallel

- Decided by: pm
- Why: with steps 1 and 2 of the end-to-end slice merged, pm split steps 3 to 5 of NOC-66 into five tickets: the relay core and SQLite backend (NOC-87), the relay's WebSocket server (NOC-89), the local Rust SDK (NOC-88), the SDK's sync (NOC-90) and the end-to-end test in the gate (NOC-91). NOC-87 and NOC-88 depend on nothing open and touch different crates, so both start now. They share only the workspace's `Cargo.toml` members list and `Cargo.lock`, a conflict the second to merge resolves by merging main. The rest are blocked in order.
- Instead of: one ticket for the relay or the SDK, which would be too large to review; or running NOC-87 and NOC-88 one after the other, which delays the slice by a day for a one-line conflict.
- Topics: factory, lanes, relay, sdk
