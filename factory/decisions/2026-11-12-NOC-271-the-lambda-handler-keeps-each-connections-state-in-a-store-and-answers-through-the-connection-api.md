# 2026-11-12 · NOC-271 · The Lambda handler keeps each connection's state in a store and answers through the connection API

- Decided by: coding agent (NOC-271), with pm's agreement on the plan
- Why: a Lambda invocation keeps nothing for the next. So the step of a connection's opening (waiting for `hello`, the challenge's nonce, or the member it acts as) and its subscriptions, each with its request id and epoch, live behind a `Connections` trait: the `connections` table on AWS, `MemoryConnections` in tests. Every answer goes out through the `ConnectionApi` trait, API Gateway's PostToConnection. The function answers every invocation with status 200 and no body, because a route response carries text, not RFC 0007's binary messages. A post that comes back gone forgets the connection. A refusal that closes posts the error first, then asks the API to close.
- Instead of: answering in the route response, which would make every binary message text or base64 and split the answers between two paths.
- Topics: relay, aws, lambda
