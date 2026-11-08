# 2026-11-08 · NOC-27 · The PR takes the skip-changes label, not a change fragment

- Decided by: coding agent (NOC-27)
- Why: the only product path the PR touches is the root `package.json`, for the `decisions` script; nothing ships to a user, and a fragment needs a component from `core`, `sdk-rust`, `sdk-ts`, `cli` or `relay`, none of which changes. `docs/release-management.md` names the label for exactly this case.
- Instead of: an `internal` fragment that names a component the PR does not touch, or a reading command without `pnpm` (`node scripts/decisions.mjs`), which the ticket asks for.
- Topics: factory, decision-log, release
