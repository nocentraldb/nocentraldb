# 2026-11-15 · NOC-320 · Discard deletes a local ad-hoc space, and only that

- Decided by: Carsten, 2026-11-15, on NOC-320 (Q9)
- Why: `Space::discard` deletes a local ad-hoc space's log, view rows, keys and copies, as RFC 0003 and RFC 0009 say; a synced space, or one that is no ad-hoc space, is refused with `NotAllowed`, until an RFC deletes spaces at the relay. It returns no notice: RFC 0009 names none. It comes with `local` (Q3).
- Instead of: a discard of a synced space on this device only, which would leave the space at the relay and on the sub-agent.
- Topics: sub-agents, ad-hoc-spaces
