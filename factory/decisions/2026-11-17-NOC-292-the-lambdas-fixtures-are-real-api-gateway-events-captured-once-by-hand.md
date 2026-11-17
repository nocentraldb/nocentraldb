# 2026-11-17 · NOC-292 · The Lambda's fixtures are real API Gateway events, captured once by hand

- Decided by: Carsten, 2026-11-13, NOC-292 (option A); carried out on 2026-11-17
- Why: one real event per route shows what API Gateway sends, which the AWS Emulator cannot (NOC-278). The owner captured them through a temporary second function, deleted afterwards (`docs/relay-deploy.md` section 3.1), so the relay itself never gains code that logs an event. Account, API, connection and request ids, the trace id, the WebSocket key and the source address are fixed stand-ins. The captured files replace the hand-written ones in `crates/ncdb-relay-lambda/src/harness/events/`, which the harness and the tests already read, so one set of events serves both.
- Instead of: an off-by-default setting in the relay that logs one redacted event per route (B), or no real fixtures (C).
- Topics: relay, aws, lambda, tests
