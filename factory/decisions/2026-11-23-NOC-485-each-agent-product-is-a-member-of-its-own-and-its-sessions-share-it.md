# 2026-11-23 · NOC-485 · Each agent product is a member of its own, with its own data directory, and the sessions of one product share it

- Decided by: Carsten, 2026-11-23, on RFC 0017's questions, through pm (NOC-488)
- Why: every reader is a visible member, so the member list must tell Claude Code and Codex apart; a member per session would fill the list.
- Instead of: one member per machine, shared by every client.
- Topics: mcp, agents, membership, rfc-0017
