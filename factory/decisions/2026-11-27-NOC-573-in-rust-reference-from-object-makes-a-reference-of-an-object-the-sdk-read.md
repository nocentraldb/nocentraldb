# 2026-11-27 · NOC-573 · In Rust, Reference::from(&Object) makes a reference of an object the SDK read

- Decided by: Carsten, 2026-11-27 (NOC-573)
- Why: A Value stays a field's value, and the conversion says plainly that only the object's space and id are taken.
- Instead of: Value::from(&Object), or no Rust form for a read object.
- Topics: rfc-0018, references, public-api, rust-sdk
