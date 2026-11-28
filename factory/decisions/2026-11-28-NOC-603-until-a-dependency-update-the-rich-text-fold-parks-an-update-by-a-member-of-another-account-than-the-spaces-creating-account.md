# 2026-11-28 · NOC-603, NOC-605 · Until a dependency update, the rich-text fold parks an update by a member of another account than the space's creating account

- Decided by: Carsten, 2026-11-28, on NOC-603, through pm
- Why: as hardening, rich text can ship before a dependency update when only the creating account's members write it; parking, not dropping, lets every member fold the parked updates and end in the same state once the update lifts the rule.
- Instead of: keeping rich text out of every release until the dependency update; or shipping it with no limit on whose updates fold.
- What this changes: the fold reads the author's account from the certificate of the genesis or of the accepted add that admitted the author under the change's generation, never from a payload; the SDKs read the field and refuse a rich-text write by a member of another account with an error of their own. For the time until the update, this replaces the NOC-468 entry's condition for a release that carries rich text; what that entry waits for still ships with the update.
- Topics: rich-text, rfc-0013, accounts, robustness
