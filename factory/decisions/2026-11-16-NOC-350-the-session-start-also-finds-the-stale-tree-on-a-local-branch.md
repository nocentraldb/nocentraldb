# 2026-11-16 · NOC-350 · The session start also finds the stale tree on a local branch, past the shallow boundary

- Decided by: coding agent (NOC-350)
- Why: the cloud clones with depth 50, and the environment's snapshot (2026-11-09, at 98ffb45) is now more than 50 commits behind main, so HEAD's history ends before the snapshot's commit and `scripts/session-start-git.mjs` left the stale tree staged in every session. The snapshot's local `main` still points at that commit. The script now resets when the index holds exactly the tree of a commit on HEAD or on a local branch; the old tree stays in that commit, so still nothing is lost. The other conditions of the NOC-52 decision stay as they were.
- Instead of: `git fetch --unshallow` or `--deepen` at every session start, which costs network time on each start and grows with the history; or rebuilding the snapshot, which only postpones the next time main outruns the depth.
- Topics: factory, cloud, git
- Supersedes: 2026-11-08-NOC-52-the-session-start-resets-the-stale-tree-only-when-nothing-is-lost.md
