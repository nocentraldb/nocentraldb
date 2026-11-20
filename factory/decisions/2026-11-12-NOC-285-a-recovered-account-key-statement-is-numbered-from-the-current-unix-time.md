# 2026-11-12 · NOC-285 · Recovery numbers its account key statement max(highest learned number + 1, current Unix seconds)

- Decided by: Carsten (2026-11-12, on NOC-285, point 2, option B)
- Why: a device that recovers from the code alone is a member of nothing, so it cannot learn the account's current number, and a number that is too low loses to the old statement and leaves the lost device its power. The current time is above every number an honest flow signed before, needs no network and stays monotonic, because later renewals count on from it; only the recovery key signs a statement, so a thief cannot inflate the number. The cost is that numbers jump (1, 2, then about 1.8 billion); RFC 0010 says "at least 1" and fits 8 bytes, so no format changes.
- Instead of: the recovering device reading the account's statements first (A), which it cannot, since it is not yet a member of the account space and possibly of no space; or another rule from the owner (C).
- Topics: accounts, recovery, rfc-0010
