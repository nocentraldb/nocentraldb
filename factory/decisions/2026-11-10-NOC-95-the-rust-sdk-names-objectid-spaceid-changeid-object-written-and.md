# 2026-11-10 · NOC-95 · The Rust SDK names ObjectId, SpaceId, ChangeId, Object, Written and page.has_more beside RFC 0009's terms

- Decided by: Carsten, 2026-11-10, on NOC-95 (RFC 0009 decision S21)
- Why: The names follow RFC 0009's Terms and the TypeScript names in snake case; `$` cannot start a Rust name, so `Object` has `id()`, `space()` and `deleted()` for `$id`, `$space` and `$deleted`, and `get::<T>(name)` reads a field; `Written` holds `change` and `ids`.
- Instead of: other names (S21 B).
- Topics: rfc-0009, sdk-rust, public-names
