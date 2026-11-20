# 2026-11-08 · NOC-11 · A model changes only additively, checked at build and on each device: no removal, rename or rule-version move

- Decided by: Carsten (RFC 0005 decisions D5, D7, D8, D11 and D12)
- Why: appending choice options and link targets is accepted, since a task board without a way to add a status is not usable for long and old clients keep the data as an unknown option or a wrong collection (D5); no version guarantees that no old client will write a removed field again (D7); v1 is simpler without aliases, which can come later as an optional key (D8); the build can be bypassed by deleting the lock or a bad merge, and the device holds the data at risk (D11); an offline member cannot be shown to run a new rule version, so moving a field to one waits for a migrations RFC, keeping RFC 0004's rule of no rule change without a migration (D12).
- Instead of: options and link targets fixed once added (D5); removing a deprecated field after some versions (D7); an alias so the TypeScript name can differ from the stored name (D8); trusting the build check alone (D11); writing the migrations RFC before v1, or allowing a new rule version once a stated share of members runs it (D12).
- Topics: rfc-0005, model-lock, schema, convergence

`docs/rfc/0005-schema-and-model-lock.md`, "The additive-change rule", lists what is accepted and refused, and "Two checks: at build and on the device" says how the SDK checks a lock against the one it last opened the database with.
