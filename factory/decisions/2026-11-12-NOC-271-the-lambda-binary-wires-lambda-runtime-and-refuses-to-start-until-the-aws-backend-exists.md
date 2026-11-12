# 2026-11-12 · NOC-271 · The Lambda binary wires lambda_runtime and refuses to start until the AWS backend exists

- Decided by: coding agent (NOC-271); the new dependencies `lambda_runtime =1.5.0` and `aws_lambda_events =1.2.1` await the owner's tick
- Why: the function's loop of invocations around the handler is written and compiles now. It prints each invocation's log line to stdout, for CloudWatch. But a working function needs the DynamoDB and S3 backend with its `connections` table (NOC-270) and a client of the real connection API (NOC-272), which pm placed in the deployment ticket. Until then, `main` says what is missing and exits with 1, so nothing deploys a relay that would lose its state between invocations. The log line is JSON and holds only typed ids, sizes, codes and times.
- Instead of: a binary that runs over SQLite in Lambda's `/tmp` and an in-memory connection store, which would look deployable but forget every connection between invocations.
- Topics: relay, aws, lambda, dependencies
