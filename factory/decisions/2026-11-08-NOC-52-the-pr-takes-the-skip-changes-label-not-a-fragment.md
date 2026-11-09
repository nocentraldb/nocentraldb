# 2026-11-08 · NOC-52 · The PR takes the skip-changes label, not a change fragment

- Decided by: coding agent (NOC-52)
- Why: the only path outside the fragment allowlist is `.claude/settings.json`, for the session start hook; nothing ships to a user, and no component from `core`, `sdk-rust`, `sdk-ts`, `cli` or `relay` changes. The same call as NOC-27.
- Instead of: an `internal` fragment that names a component the PR does not touch.
- Topics: factory, release
