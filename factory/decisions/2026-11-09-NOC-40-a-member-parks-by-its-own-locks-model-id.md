# 2026-11-09 · NOC-40 · A member compares payload key 64 with the model.id of its own lock, pending the owner

- Decided by: coding agent (NOC-40), pending the owner: pm put it to the owner as a question in #67, with option B, which pm leans to; the text stays at A until the owner answers
- Why: a member folds by its own `model.lock.json`, and RFC 0005's D13 B parks a change so that no member folds it by the wrong rule, which is relative to that lock; every member that may write runs the genesis's model (D13 A), so all of them park the same changes, and the state stays a function of the change's bytes and the member's lock, as step 7's already is. The check sits in step 6 of the step table, so the table keeps its steps. A change without key 64 folds as before, because the key is "may ignore".
- Instead of: B, comparing with the model.id the space's genesis records (key 13), which is the same on every copy, so a misconfigured or outdated member parks the same changes as every other; a member whose lock differs from the genesis is then a separate problem.
- Topics: rfc-0002, rfc-0005, log, model, convergence
