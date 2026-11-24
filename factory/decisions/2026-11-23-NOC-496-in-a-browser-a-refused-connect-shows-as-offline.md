# 2026-11-23 · NOC-496 · In a browser a connection the relay refuses shows as offline, without `connect_refused`

- Decided by: coding agent (NOC-496); pm to confirm, since GENESIS principle 7 says behaviour never forks by platform
- Why: the browser's `WebSocket` reports a failed handshake only as `error` and `close` with code 1006; it never exposes the HTTP status, so the core in a browser cannot tell a 403 from an unreachable relay. The TypeScript SDK runs in a browser, so it never reports `connect_refused`; its docs and the change fragment say so plainly. The relay function is still called before each connect, so a fresh token reaches the next connect on every platform.
- Instead of: probing the relay's endpoint with `fetch()` after a failed connect, which CORS makes opaque and which an authorizer need not answer the same way; or guessing `connect_refused` after repeated failures, which would claim more than the SDK knows.
- Topics: sdk-ts, sync, relay, browser
