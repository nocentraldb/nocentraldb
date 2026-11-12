# 2026-11-12 · NOC-271 · A local stand-in for API Gateway runs the handler and the SDK's sync suite without AWS

- Decided by: coding agent (NOC-271), with pm's agreement on the plan
- Why: the AWS Emulator serves no WebSocket data plane (NOC-278). `harness::Gateway` (the feature `harness`) listens for WebSocket connections. It turns each open, message and close into an event built from a recorded one and read back through `aws_lambda_events`, and it writes the handler's posts to the sockets. Like API Gateway, it closes a connection above 128 KiB (1009) or at its maximum age (1001), then sends `$disconnect`. A second port serves the URLs of large changes. The SDK's test `serve()` picks it when `NCDB_TEST_RELAY=lambda`, and the moon task `ncdb-relay-lambda:test-sdk-sync` runs the SDK's sync suite that way in `pnpm gates`. The recorded events follow the shape of real API Gateway events; none is captured from a deployed API, because there is none yet.
- Instead of: a mock that calls the handler with hand-built `Event` values, which would skip the JSON Lambda reads and the sockets the SDK uses; or a second copy of the SDK's sync tests.
- Topics: relay, aws, lambda, tests
