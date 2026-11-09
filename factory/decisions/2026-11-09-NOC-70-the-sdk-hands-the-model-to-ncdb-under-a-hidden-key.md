# 2026-11-09 · NOC-70 · The SDK hands the model to ncdb under a hidden key, and exports no helper types

- Decided by: coding agent (NOC-70)
- Why: RFC 0005 approves only `nc` and its members as public names. `nc.model` returns the collections as declared (so `model.tasks` works) and keeps a function that describes the model under `Symbol.for("nocentraldb.model.describe")`, a non-enumerable key that no app sees or types; `ncdb model build` calls it. The types the field functions return (the field with its modifiers, a collection, a name, an index) are not exported, so tsc writes them into the API report without `export` and an app cannot name them. The core, not the SDK, checks the model's rules, so the build and the device check agree.
- Instead of: an exported `describe` function or exported types such as `Field`, which would be public names RFC 0005 does not list.
- Topics: sdk, api, schema, rfc-0005
