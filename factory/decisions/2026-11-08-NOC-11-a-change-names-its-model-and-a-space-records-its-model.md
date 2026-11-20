# 2026-11-08 · NOC-11 · A change names its writer's model id and version, a space records its model, and no space folds two models

- Decided by: Carsten (RFC 0005 decisions D10 and D13)
- Why: the writer's model version in the payload, a "may ignore" key inside the ciphertext, lets an old client report which model version it needs to read its parked changes, at a few bytes the relay cannot see (D10). Two apps of different `model.id`s writing one space would make members merge one field by two rules: a space records the `model.id` it was created with in its first control entry, and the SDK refuses to write into a space of another model, which stops honest apps at the first write; each change also carries its writer's `model.id`, and a member parks a change of another model whole, so a buggy or hostile client's changes park the same way on every member (D13, option C).
- Instead of: no model version in changes, reporting only that changes are parked and why (D10); only the space's record, only the `model.id` in each change, or allowing two models and documenting the risk (D13).
- Topics: rfc-0005, rfc-0002, model, convergence

`docs/rfc/0005-schema-and-model-lock.md`, "Data from a newer model", states the rules; the payload key's layout is in `2026-11-09-NOC-40-the-model-is-payload-key-64-one-map-of-id-and-version.md`.
