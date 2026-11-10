---
type: internal
tickets: [NOC-93]
components: [core]
---
The core encodes, decodes, merges and resolves the field type `ncdb/ref`, reads it from `model.lock.json`, and the SQLite view, not yet released, gives it its 32-byte index column; the SDKs do not offer `nc.ref` yet.
