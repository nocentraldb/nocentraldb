# 2026-11-20 · NOC-390 · The relay keeps a row for a member resolve no longer names, as a non-member that admits no change

- Decided by: coding agent, 2026-11-20 (NOC-390)
- Why: RFC 0008 makes the relay's membership table `resolve` of its log. When a control change drops a member out of `resolve` entirely (a removal excluded its every add), the relay core writes that member's row again with no role, grant or revocation and `current` false, keeping only its keys and certificate. Upload, pull and `members_of` then refuse it, and no sealed change of it is admitted. The fix stays in the core: a backend still only upserts the rows it is given, so the SQLite and the AWS backend need no change and the backend interface gains no delete effect. A later add of the same member writes its row again.
- Instead of: a new effect that deletes the row, which every backend would have to implement and the conformance suite to test.
- Topics: relay, membership
