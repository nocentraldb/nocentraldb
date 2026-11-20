# 2026-11-09 · NOC-64 · A read is a plain nc.Infer object with $-properties and nc.Unknown, and space.inspect gives the merge details

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decision S5)
- Why: Most reads show values and conflicts are rare, so a read object carries `$id`, `$space`, `$deleted`, `$conflicts` and `$raw`, and `space.inspect` gives every detail. Unknown choice options are common after an additive change, so `nc.Unknown` makes the app handle them; a value outside the field's kind comes only from buggy or hostile members, so it reads `undefined` and `$raw` names it (principle 3) instead of typing every field as possibly anything.
- Instead of: every field wrapped as `{ value, conflicts }` in every read (S5 B); a value outside the kind returned as it is, typed `unknown` (S5 C).
- Topics: rfc-0009, sdk, api, types
