# 2026-11-09 · NOC-70 · A link target must be a collection of the model

- Decided by: coding agent (NOC-70), for pm's review
- Why: RFC 0005 refuses an index on an unknown field because the core could not build it; a link whose target names no collection of the model is the same kind of mistake (a typo in `nc.link("peple")`), and every link into it would read as wrong collection forever. The build refuses it with "target X is no collection of the model", next to the RFC's other checks of a lock on its own (unknown built-in rule version, plugin type without its plugin, an option listed twice).
- Instead of: accepting such a lock and leaving the developer to find the typo from link statuses at run time.
- Topics: model-lock, rfc-0005, links
