# 2026-11-13 · NOC-176 · The TypeScript space status names its reasons in camel case and an erasure's age in milliseconds

- Decided by: Carsten, 2026-11-13, on #317 (Authorship ticked)
- Why: RFC 0009 lists the twelve values of `space.status` and describes `parked` and `rejected` per reason in prose. The TypeScript `SpaceStatus` holds them in camel case, with Rust's `newestModel` and `badPersonKeyRecords`. `parked` and `rejected` are `Record<reason, count>`, keyed by the Rust enums' names in camel case, such as `anotherModel` and `badSignature`, the way the bridge already names error codes. A provisional erasure's `age` is a number of milliseconds, JavaScript's unit of time. Ids are lowercase hex, and `performer` is the member id's 64 digits (NOC-298).
- Instead of: reasons as arrays of `{reason, count}`, which an app cannot index by reason; or `age` in seconds, which loses the precision the Rust `Duration` holds.
- Topics: sdk-ts, status
