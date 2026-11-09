# 2026-11-08 · NOC-48 · Checks::verify and Checks::open return where a change stops, and open returns the payload

- Decided by: coding agent (NOC-48)
- Why: `seal::open` decrypts and decodes in one call and returns a `Payload`, so a `Checks::open` that returned plaintext bytes would have to encode the payload again. `verify` and `open` now return `Result<_, Stop>`, where `Stop` is parked or rejected with the log's own reason, so the `OpenError` table feeds both and `receive` turns a `Stop` into the state without a second mapping. The `Verified` and `Opened` enums are gone. `ncdb-core` is unpublished, so no client depends on them, but cargo-semver-checks reports the change as breaking, so the PR carries a `breaking` fragment with a Migration section.
- Instead of: keeping `Opened::Plaintext(Vec<u8>)` and re-encoding the decoded payload, or adding variants to `Verified` and `Opened` for each way `seal` can fail.
- Topics: rfc-0002, log, api
