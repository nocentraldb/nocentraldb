# ncdb-relay-lambda

The NoCentralDB relay as one AWS Lambda function behind an API Gateway WebSocket API, as [RFC 0007, "The AWS backend"](../../docs/rfc/0007-relay.md#the-aws-backend) describes it. API Gateway invokes the function for `$connect`, `$disconnect` and `$default`. The function runs the relay core of `ncdb-relay` on each message and sends its answers through API Gateway's connection API. The same API carries live entries to subscribers and `added` to the open connections of a member a push added.

The function checks nothing itself. Every request goes through the relay core, the same as in the WebSocket server `ncdb-relay-server`, and both speak the same messages: the core's `wire` and `page`.

**Not deployable yet.** The DynamoDB and S3 backend exists (`ncdb-relay-aws`, NOC-270). The binary also needs a `connections` store on DynamoDB and a client of the real connection API (NOC-272). Until those exist, it says so and exits. The handler runs on the SQLite backend and the in-memory connections, in this crate's tests and in the local harness.

## What it keeps between invocations

A Lambda invocation remembers nothing for the next. What the WebSocket server keeps in a connection's task, the handler keeps in a `Connections` store:

- the step of each connection's opening (waiting for `hello`, the nonce of its challenge, or the member it acts as);
- each subscription, with its request id and relay epoch.

On AWS this store is the `connections` table, whose TTL clears a record 2 hours after its connection opened. Here it is `MemoryConnections`.

## Limits

- **128 KiB per message.** API Gateway refuses a larger message and closes the connection. The handler posts no message above 128 KiB: a frame that does not fit inline comes as a download URL, and an answer that does not fit is `too_large`.
- **2 hours per connection.** API Gateway closes every connection after 2 hours and then sends `$disconnect`, which forgets the connection and its subscriptions. The member reconnects and resubscribes (RFC 0007, "Liveness").
- **Best-effort delivery.** A failed post is not retried, as RFC 0007 says. A post to a connection that is gone forgets that connection.

## Logs

Each invocation prints one JSON line, which Lambda sends to CloudWatch Logs. It holds:

- the route and the connection id;
- the member id, once the connection acts as a member;
- the message's type;
- the bytes in and out, and the number of posts;
- the codes of errors and refused frames;
- when it ran and how long it took.

It holds no frame, key, signature or client string: every field is typed as an id, a size, a code or a time.

## The local harness

With the feature `harness`, `harness::Gateway` stands in for API Gateway on the local machine:

- It listens for WebSocket connections.
- It turns each open, message and close into the event API Gateway sends. Each event is built from a recorded one in `src/harness/events/` and read back through `aws_lambda_events`, as Lambda reads it.
- It calls the handler with each event, and writes the handler's posts to the sockets.
- Like API Gateway, it closes a connection that sends a message above 128 KiB, closes every connection at its maximum age (2 hours, or shorter in tests), and hands each connection's messages to the handler one at a time.
- A second port serves the upload and download URLs of large changes, which S3's presigned URLs serve on AWS.

The recorded events follow the shape of real API Gateway events. They are not captured from a deployed API: the AWS Emulator serves no WebSocket data plane (NOC-278), and the relay is not deployed yet.

The Rust SDK's sync suite runs through the harness when `NCDB_TEST_RELAY=lambda` is set. The task `moon run ncdb-relay-lambda:test-sdk-sync` does this, and it is part of `pnpm gates`.
