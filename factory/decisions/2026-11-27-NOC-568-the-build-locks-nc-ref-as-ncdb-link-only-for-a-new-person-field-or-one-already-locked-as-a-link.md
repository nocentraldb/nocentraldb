# 2026-11-27 · NOC-568 · The build locks nc.ref as ncdb/link only for a new field a personal field names as person, or one already locked as ncdb/link

- Decided by: coding agent (NOC-568)
- Why: A field locked as ncdb/ref may already hold references into other spaces and cannot change its type, so the build keeps it and refuses it as a person, naming the personal field.
- Instead of: Turning a field locked as ncdb/ref into ncdb/link when a personal field names it, which the type-change rule refuses anyway with a less precise message.
- Topics: rfc-0018, references, personal-fields, model-lock
