# 2026-11-08 · NOC-11 · model.lock.json is canonical JSON with a model version, a content hash, a lineage id and since per element

- Decided by: Carsten (RFC 0005 decisions D3 and D4)
- Why: a reviewer reads a change to the model in the pull request, and every runtime parses JSON the same way; the number is what a person and an error message can use ("needs model version 7"), the hash says whether two locks describe the same model, the lineage id (`model.id`) catches a lock that was started over, and `since` lets an old client say which version it is missing.
- Instead of: CBOR like the changes, or YAML (D3); a content hash only, or semantic versions (D4).
- Topics: rfc-0005, model-lock, on-disk-format

`docs/rfc/0005-schema-and-model-lock.md`, "The structure of model.lock.json", states the keys and the canonical form.
