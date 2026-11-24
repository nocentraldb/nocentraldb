# 2026-11-23 · NOC-485 · The first ncdb-mcp stdio session hosts the copy when no serve runs, and the host ends with its last client

- Decided by: Carsten, 2026-11-23, on RFC 0017's questions, through pm (NOC-488)
- Why: it works without any app or service set up, while one process still holds each database.
- Instead of: only ncdb-mcp serve hosts, and stdio only forwards.
- Topics: mcp, agents, rfc-0017
