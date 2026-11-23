# 2026-11-23 · NOC-469 · The AWS backend keeps an identity's admission in `spaces` under the identity and the byte `a`

- Decided by: the coding agent of NOC-469
- Why: as the statement item of NOC-420, a 33-byte key collides with no 16-byte space id, no 32-byte guest item, no reserved item and, by its last byte, no statement item, so no stack creates a new table. `admit` is one `UpdateItem` that sets `until`, `spaces` and `admission_key` and keeps `created` with `if_not_exists`; a genesis append raises `created` in its own transaction on the condition `created < spaces`, so two appends at once cannot exceed the admission, and a failed condition answers `AdmissionSpent`. SQLite keeps a table `admissions` with the same columns.
- Instead of: a table of its own, which every stack would have to create; the count on the guest item, which one genesis would have to write twice.
- Topics: relay, aws, admission, rfc-0015
