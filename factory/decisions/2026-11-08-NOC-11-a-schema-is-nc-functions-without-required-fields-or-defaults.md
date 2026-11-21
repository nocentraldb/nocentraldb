# 2026-11-08 · NOC-11 · A schema is one TypeScript value of nc.* functions with chained modifiers, with no required fields or defaults

- Decided by: Carsten (RFC 0005 decisions D1, D2 and D6)
- Why: functions under one namespace need no compiler setting and let TypeScript infer the app's types from the model; approving the names in RFC 0005 lets the SDK and `ncdb` be built against them; an object written under an older model lacks newer fields, so "unset" exists anyway, and a required field cannot be enforced on data from other members.
- Instead of: classes with decorators, or a schema file in a language of its own (D1); approving only RFC 0001's names and deciding the new ones in the SDK's API review (D2); defaults recorded in the lock, or required fields the SDK enforces on write (D6).
- Topics: rfc-0005, schema, public-names, sdk

`docs/rfc/0005-schema-and-model-lock.md` states the rules: "The schema in TypeScript" (with "No required fields and no defaults") and "Public names".
