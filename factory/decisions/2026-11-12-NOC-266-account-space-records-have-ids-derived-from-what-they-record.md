# 2026-11-12 · NOC-266 · An account-space record's object id derives from what it records, and the shipped model has one index per collection

- Decided by: Carsten, 2026-11-12, on #284
- Why: each device keeps its own records in the account space current and must find them again in any copy, also one it reopens or pulls fresh. A member record's id is the first 16 bytes of SHA-256 of `"nocentraldb/account-member"` and the member id; a membership record's, of `"nocentraldb/account-membership"`, the member id and the space id. A device that leaves a space deletes its record and restores the same object when it joins again. The shipped model (`crates/nocentraldb/src/account/model.lock.json`, `model.id` `c459aa69312cf983832681fb08068b60`) has RFC 0010's five collections and fields, and one index per collection in the space (`byMemberId`, `bySpace`, `byAccount`, `byNumber`), which the SDK reads the records through; RFC 0010 names no index.
- Instead of: random ids with a lookup by field, which would need the index to answer "is my record there" before every write and could leave two records for one membership when two writes race on a device.
- Topics: accounts, sdk-rust, model-lock
