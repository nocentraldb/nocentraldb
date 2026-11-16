# 2026-11-16 · NOC-350 · The stale-tree warning goes to the agent, and the hook still exits 0

- Decided by: coding agent (NOC-350)
- Why: the ticket asks the session start to fail loudly when the tree is not clean. A SessionStart hook's stdout on exit 0 goes into the agent's context; on a non-zero exit, Claude Code shows stderr only to the user and the agent never reads it. The agent is the one who would commit the stale tree, so the hook prints a warning that opens with `WARNING`, counts the staged paths and says a commit now would revert them, and exits 0 as before.
- Instead of: exiting non-zero, which would hide the warning from the agent that must act on it.
- Topics: factory, cloud, git
