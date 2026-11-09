# 2026-11-09 · NOC-65 · The session start installs dependencies when the lockfile's hash differs from its own record

- Decided by: coding agent (NOC-65)
- Why: `scripts/session-start-deps.mjs` keeps the SHA-256 of the `pnpm-lock.yaml` it last installed from in `node_modules/.session-start-deps` and runs `pnpm install --frozen-lockfile` when that record is missing or differs. A fresh snapshot has no record, so its first session installs once, which costs a few seconds when nothing changed; after that, sessions skip the install until the lockfile changes. It covers pnpm only, as the ticket's acceptance criteria ask; Cargo resolves missing crates itself on the next build.
- Instead of: comparing `pnpm-lock.yaml` with pnpm's own copy in `node_modules/.pnpm/lock.yaml`, which pnpm writes in another form (it is not byte-equal even right after an install), or running `pnpm install` at every session start.
- Topics: cloud, session-start, dependencies, factory
