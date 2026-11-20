# 2026-11-08 · NOC-52 · The session start resets the stale tree only when nothing is lost

- Decided by: coding agent (NOC-52)
- Why: `scripts/session-start-git.mjs` runs `git reset --hard HEAD` only in a cloud session, and only when the index holds exactly the tree of a commit in HEAD's history, the working tree matches the index and nothing is untracked, so the old tree stays in that commit and nothing else exists to lose. AGENTS.md's cloud block, hidden by skip-worktree, is kept on top of HEAD's AGENTS.md. In every other case the hook changes nothing and tells the agent to check `git status`.
- Instead of: stashing the stale tree at every start, which piles a stash per session onto a state git already holds, or always resetting, which could discard an agent's own work if a session restarts mid-ticket.
- Topics: factory, cloud, git
