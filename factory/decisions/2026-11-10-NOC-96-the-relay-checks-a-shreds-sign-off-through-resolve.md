# 2026-11-10 · NOC-96 · The relay checks a shred's sign-off through `resolve`, and keeps the record until a later ticket deletes it

- Decided by: coding agent (NOC-96)
- Why: RFC 0008 makes "a shred names a person key that exists and carries a valid sign-off" one of step 1's own rules, so the sign-off is checked inside `resolve`, in the shred's causal past, and the relay, which accepts a control change when `resolve` over its log accepts it, makes the same check as every member. The sign-off verifies for the space, the person key and the shred's author as performer; its account must hold a current admin in the causal past and must not be the account of the performer's certificate. A sub-agent's certificate carries its parent's account key, so it counts as its parent's account without a rule of its own. A shred that fails is `membership_rule`, as every rejected control change. The relay's deletion of the person-key record once the shred is stable (M20) is not built: the record stays stored, and the relay accepts no new record of a shredded id.
- Instead of: a separate sign-off check in the relay core, which could drift from the members' fold; or deleting the record when the shred is appended, which M20 rejects.
- What this does not do: a stored record of a shredded person key stays on the relay until stability is computed and the deletion is requested.
- Topics: relay, rfc-0007, rfc-0008, shred, membership
- Supersedes: 2026-11-10-NOC-87-the-relay-refuses-shred-entries-until-it-checks-their-sign-off.md
