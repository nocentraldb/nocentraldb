# 2026-11-12 · NOC-270 · The AWS store runs each call on the Tokio runtime it is given, and sweeps by scanning

- Decided by: proposed by the coding agent (NOC-270); the owner decides with the tick on PR #275
- Why: the AWS SDK for Rust needs a Tokio runtime, the backend interface's futures must run on any executor (the conformance suite polls them with no runtime), so each call is spawned on a runtime handle the caller passes; and `begin_upload` must delete what expired in the same call, which on DynamoDB is a scan of `uploads` and `tokens`, small tables of live URLs, with a TTL attribute on every item for what a sweep misses.
- Instead of: requiring callers to poll inside a Tokio runtime, which the suite and other servers would have to know; or relying on DynamoDB's TTL alone, which deletes up to days late.
- Topics: relay, aws, uploads
