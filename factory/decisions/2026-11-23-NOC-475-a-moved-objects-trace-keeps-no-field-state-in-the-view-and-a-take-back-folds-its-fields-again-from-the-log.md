# 2026-11-23 · NOC-475 · A moved object's trace keeps no field state in the view, and a take-back folds its fields again from the log

- Decided by: coding agent, NOC-475
- Why: RFC 0016 keeps no plaintext of a trace's fields, as for an erased field; dropping the merge states too, and folding them again from the log when the move is taken back, keeps each field's state a function of its operations, so copies converge in any order of arrival. A field operation that arrives while the object is a trace is decoded, so it parks as on every copy, but not stored.
- Instead of: keeping the merge states and clearing only the read values, which leaves every earlier value in plaintext on the device.
- Topics: rfc-0016, move, view, convergence, plaintext
