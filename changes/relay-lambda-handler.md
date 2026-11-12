---
type: internal
tickets: [NOC-271]
components: [relay]
---
A new crate, `ncdb-relay-lambda`, runs the relay core as the handler of API Gateway's WebSocket events, with a local stand-in for API Gateway; the Rust SDK's sync suite also runs through it. Not deployable until the AWS backend and the connection API client exist.
