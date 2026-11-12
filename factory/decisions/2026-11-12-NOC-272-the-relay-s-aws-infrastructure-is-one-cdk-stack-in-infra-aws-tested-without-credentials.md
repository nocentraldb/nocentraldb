# 2026-11-12 · NOC-272 · The relay's AWS infrastructure is one CDK stack in `infra/aws`, tested without credentials

- Decided by: coding agent (NOC-272), following NOC-264 (point 1: CDK in TypeScript, in `infra/aws`) and pm's answers on the plan
- Why: one stack holds RFC 0007's AWS backend, the deployment's values arrive as CDK context, and the tests check it all with no AWS credentials.
  - **The resources.** The WebSocket API with its three routes and one stage; the function (arm64, 256 MB, `provided.al2023`, the `bootstrap` that cargo-lambda builds from `ncdb-relay-lambda` with the feature `aws`); the eight tables; the bucket; the log group kept 14 days; and alarms and a 20 USD budget that mail the owner.
  - **The tables:** RFC 0007's five, NOC-270's `uploads` and `tokens`, and `connections`. Each is on demand, has point-in-time recovery for 35 days, and is kept when the stack is deleted.
  - **The bucket:** all public access blocked, S3-managed encryption, no versions, TLS only, uploads deleted after a day, and CORS for any origin, so browsers can use the presigned URLs (NOC-245 A).
  - **The deployment's values** are `relayId`, `allowedAccounts` and `alarmEmail`, given as context. The deploy workflow passes them from the GitHub environment `aws-prod`, so no address or id lives in the repository.
  - **The tests** synthesize the stack with no AWS credentials and check every table's keys, indexes, billing, recovery and TTL against what the backend's own schema code creates. They also check the bucket's settings, the function, the routes, the role's actions (no wildcard and no IAM), the log retention, the alarms and the budget.
  - **The snapshot.** A committed snapshot of the template is how a pull request shows its diff against the deployed infrastructure without credentials (pm, answer C).
  - **Reporting off.** CDK's version reporting and the CLI's telemetry are off, as the emulator's is.
- Instead of: table names CloudFormation generates, which the function could not derive from one prefix; the deployment's values committed in `cdk.json`, which would put the owner's address in the repository; or a `cdk diff` against the real account on pull requests, which needs credentials that pull requests do not get.
- Topics: relay, aws, infrastructure, cdk
