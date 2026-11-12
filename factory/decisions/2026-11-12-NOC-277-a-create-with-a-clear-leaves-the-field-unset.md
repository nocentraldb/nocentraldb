# 2026-11-12 · NOC-277 · A create with a clear leaves the field unset

- Decided by: pm, NOC-277, on the coding agent's proposal; `Value::clear()` itself is the owner's (Carsten, 2026-11-12, NOC-277, A)
- Why: RFC 0009 says `null` clears a field, and the Rust SDK's `Value::clear()` does the same, but neither says what a clear in a create does. A new object has no value in that field, so writing "no value" there changes nothing visible: the field reads unset, as a TypeScript `null` on create does. Allowing it lets an app pass the same values map to a create and an update. A clear of a set or a list of links is refused with `invalidValue` in either, since RFC 0009 defines `null` only for the fields a set (1) writes: basic values, links and references.
- Instead of: refusing a clear in a create with `invalidValue`, which would make an app strip clears from a map it builds once for both calls.
- Topics: sdk-rust, write, api
