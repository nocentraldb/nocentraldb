# 2026-11-23 · NOC-485 · Declared tools may read and act (create, update, delete), and --read-only turns the actions off

- Decided by: Carsten, 2026-11-23, on RFC 0017's questions, through pm (NOC-488)
- Why: an agent that only reads covers few of the tasks apps want; the member's role and --read-only keep writes out where they are not wanted.
- Instead of: reads only first, with writes in a later step.
- Topics: mcp, agents, rfc-0017
