# 2026-11-09 · NOC-40 · A member parks a change whose key 64 does not name the model.id of its own lock, or is malformed

- Decided by: coding agent (NOC-40), reading RFC 0005's D13 B; flagged to pm for review
- Why: a member folds by its own `model.lock.json`, and D13 B parks a change so that no member folds it by the wrong rule, which is relative to that lock; every member that may write runs the genesis's model (D13 A), so all of them park the same changes, and the state stays a function of the change's bytes and the member's lock, as step 7's already is. A key 64 of the wrong shape does not show the member's model, so it parks with the same reason "another model". The check sits in step 6 of the step table, so the table keeps its steps. A change without key 64 folds as before, because the key is "may ignore".
- Instead of: comparing with the space's model id from the genesis, under which a member whose lock is of another model would fold the space's changes by its own, wrong rules; or treating a malformed key 64 as absent, which would fold a change no lock vouches for.
- Topics: rfc-0002, rfc-0005, log, model, convergence
