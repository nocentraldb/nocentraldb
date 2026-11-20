# 2026-11-12 · NOC-232 · The TypeScript SDK runs no plugins in v1; ncdb core build comes next, and run-time wasm plugins stay open

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decision 4
- Why: A TypeScript member treats every plugin type as not registered, keeps and forwards its changes, so nothing is lost and the merge does not fork; only what an app can show differs from a Rust member, a principle 7 conflict the owner accepted. `ncdb core build` will build an app's own wasm core with its plugins, at the cost of a Rust toolchain on the developer's machine.
- Instead of: wasm plugin modules every core loads at run time, which reverses RFC 0004's decision against run-time code and needs a deterministic limit per call; it stays possible later with the owner's decision.
- Topics: rfc-0011, plugins, sdk-ts, wasm
