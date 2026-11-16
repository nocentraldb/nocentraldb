# 2026-11-16 · NOC-350 · The session start hooks run their scripts as HEAD holds them, not the working tree's copies

- Decided by: coding agent (NOC-350)
- Why: a restored session's working tree is the snapshot's (2026-11-09, at 98ffb45), so `node scripts/session-start-git.mjs` ran the snapshot's copy of the script, and #371's fix never ran in any session. This session saw it: the hook printed NOC-52's old message, and the working tree's script differed from HEAD's. HEAD's script, run against this container's real state (1,498 paths, tree 4d652f4), resets it to a clean `git status`. Both hooks in `.claude/settings.json` (`session-start-git.mjs` and `session-start-deps.mjs`, which had the same stale copy) now run `git show HEAD:scripts/<script>` through `node --input-type=module -e`, which keeps the hook's input on stdin.
- Instead of: keeping `node scripts/<script>` and rebuilding the snapshot after every change to either script.
- Topics: factory, cloud, git
