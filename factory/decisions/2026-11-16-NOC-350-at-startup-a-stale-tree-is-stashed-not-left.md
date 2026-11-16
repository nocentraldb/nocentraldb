# 2026-11-16 · NOC-350 · At a session's startup, a stale tree that matches no commit is stashed under a named stash and reset

- Decided by: coding agent (NOC-350), as pm proposed on the ticket
- Why: a fresh checkout holds no work of its own, so a tree the hook cannot match to a commit is still the snapshot's. Stashing it (with untracked files) under a stash named `session-start-git: tree <id> staged over <HEAD> at session start` loses nothing and leaves a clean `git status`. Only when the hook's source is `startup`; on resume, clear or compact the agent may have staged its own work, so the hook still leaves it and warns. The cloud block on top of AGENTS.md is kept as in a reset.
- Instead of: leaving every unmatched tree alone with a warning, which made every session stash by hand.
- Topics: factory, cloud, git
