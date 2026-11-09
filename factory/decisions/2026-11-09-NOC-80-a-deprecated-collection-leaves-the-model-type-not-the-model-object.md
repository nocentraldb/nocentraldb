# 2026-11-09 · NOC-80 · A deprecated collection leaves the model's type, not the model object

- Decided by: coding agent (NOC-80)
- Why: RFC 0005 D17 hides a deprecated collection from the SDK's TypeScript types, and RFC 0009 refuses writes into it with `deprecated`. `nc.model` returns a type without it, so typed code cannot name it, but the object still holds it at run time, so a write from untyped code can be refused with `deprecated` rather than fail on `undefined`. A collection's draft takes `deprecated` with the reason, as a field's does; the core keeps the version that first deprecated it, as for a field, and `ncdb model build` lists it as `~ collection <name> deprecated: <reason>`.
- Instead of: removing the collection from the object `nc.model` returns, which would leave a later write no collection to refuse.
- Topics: sdk, schema, model-lock, rfc-0005
