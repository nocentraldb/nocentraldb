# 2026-11-09 · NOC-64 · The SDK keeps RFC 0004's read names, names the roles reader, writer and admin, and throws one nc.Error with codes

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decisions S6, S13 and S17)
- Why: RFC 0004's working names (`value`, `conflicts`, `lostRemovals`, `outcome` with `won`, `lost`, `replaced`, `applied`, the statuses `ok`, `deleted`, `missing`, `wrongCollection`) are plain words the RFCs and the FAQ already use. The roles say what each may do, and "owner" would suggest one person while a space has many admins. One error class with a string `code` and the ids it concerns is easy to match on in both SDKs, maps one to one to Rust's `ErrorCode`, and keeps the API small.
- Instead of: other read names such as `losers` and `dangling` (S6 B); the roles `viewer`, `editor`, `owner` (S13 B); a class per error (S17 B).
- Topics: rfc-0009, sdk, public-names, roles, errors

The full list stands in `docs/rfc/0009-sdk-api.md`, "Public names" and "Errors".
