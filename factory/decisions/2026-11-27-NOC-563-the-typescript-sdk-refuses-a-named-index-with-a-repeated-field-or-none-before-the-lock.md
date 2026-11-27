# 2026-11-27 · NOC-563 · The TypeScript SDK refuses a named index with a repeated field or no field before the lock, and the core's lock check stays as it is

- Decided by: coding agent (NOC-563)
- Why: Only the named form knows where and orderBy apart, and a new refusal in the core's lock check would refuse locks that today's form already built.
- Instead of: A refusal of repeated index fields in ncdb-core's lock check, for both forms.
- Topics: rfc-0018, indexes, schema, model-lock
