# 2026-11-15 · NOC-320 · Local ad-hoc spaces come later; synced ones first

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q3 c)
- Why: a local ad-hoc space never reaches the relay, but the sub-agent has a database of its own, and no RFC says how changes travel between the two on one machine. NOC-318 builds synced ad-hoc spaces; `local` and `discard` are a ticket of their own.
- Instead of: a sub-agent handle on the parent's own database, sharing its store; or a local transport between two databases, a loopback relay or an exchange of change files, which would be a new wire or IPC surface.
- Topics: sub-agents, ad-hoc-spaces, sync
