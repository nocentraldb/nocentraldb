# 2026-11-09 · NOC-40 · A member compares payload key 64 with the model.id of its own lock

- Decided by: Carsten, 2026-11-09, on the question pm put in #67 (option A)
- Why: in a correct setup comparing with the member's own lock and comparing with the genesis's model id (key 13) give the same result; they differ only for a member whose lock does not match the genesis, which needs RFC 0005's open-time check on the device either way. If that check is ever missing, A stays safe, because such a member parks every change of the space, while B would fold them by the wrong rules. Under A, a member folds by its own `model.lock.json` and parks every change whose key 64 names another `model.id`, at step 6 of the step table, so the table keeps its steps; a change without key 64 folds as before, because the key is "may ignore".
- Instead of: B, comparing with the model.id the space's genesis records, which pm had leaned to because it is the same on every copy.
- Topics: rfc-0002, rfc-0005, log, model, convergence
