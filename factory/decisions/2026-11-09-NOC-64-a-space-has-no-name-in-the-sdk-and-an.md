# 2026-11-09 · NOC-64 · A space has no name in the SDK, and an app keeps space names in its own model

- Decided by: Carsten, 2026-11-09, in chat with pm, on #78 (RFC 0009 decision S11)
- Why: The genesis holds no name, because a name is content the relay must not read. A reserved core collection would not be in the app's lock, so RFC 0002's fold would park it as unknown; an app already models what a project is, and its name belongs there. Names can come later as an addition.
- Instead of: a reserved core collection in each space, such as `$space` with `name` and `description` (S11 B).
- Topics: rfc-0009, sdk, spaces, names
