# 2026-11-12 · NOC-232 · A plugin implements a typed FieldType trait with decode_state and write, which the core wraps over bytes

- Decided by: Carsten, 2026-11-12, on NOC-232 ("Ich stimme den Vorschlägen zu"), RFC 0011 decision 1
- Why: The author writes Rust types for the state, the operation and the read value, and the core keeps a boxed adapter over the bytes `fs_<collection>` stores; `decode_state` reads a stored state back and must round-trip, and `write` turns the app's write intent into operations, because RFC 0004's contract has neither. Values cross the SDKs as `Data`, RFC 0002's CBOR subset.
- Instead of: a trait over bytes only, where every author would encode CBOR by hand; extending the core's closed list of types per app by macro or feature flags, which makes the core's API depend on the app.
- Topics: rfc-0011, plugins, field-types, api
