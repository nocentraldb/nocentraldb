# 2026-11-12 · NOC-271 · RFC 0007's messages and their `entries` pages live in the relay core

- Decided by: coding agent (NOC-271), with pm's agreement
- Why: the Lambda function behind API Gateway speaks the same messages as the WebSocket server, so the codec (`wire`) and the paging of `entries` under 128 KiB (`page`) move from `ncdb-relay-server` into `ncdb-relay`, unchanged. The server re-exports `wire` and its constants, so its public names stay. `page::read` no longer runs on a blocking thread, and the deployment passes in how it makes a download URL from a token.
- Instead of: the Lambda crate depending on `ncdb-relay-server`, which links tokio-tungstenite and the server's listener into the function, or a copy of the codec, which would drift from the server's.
- Topics: relay, aws, wire
