# 2026-11-20 · NOC-362 · Plugin field types are Rust only in v1

- Decided by: Carsten, 2026-11-20, on NOC-362 ("Ja zu den Empfehlungen")
- Why: only the Rust SDK registers plugins, and the TypeScript SDK has none in v1 (RFC 0011, "TypeScript"). RFC 0005 says so where a schema uses a plugin's field type, and its example marks the field.
- Instead of: leaving open how a TypeScript model declares a plugin field.
- Topics: plugins, schema, sdk-ts, rfc-0005
