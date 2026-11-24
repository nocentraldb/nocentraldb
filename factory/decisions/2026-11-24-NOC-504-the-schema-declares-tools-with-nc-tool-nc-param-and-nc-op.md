# 2026-11-24 · NOC-504 · The schema declares agent tools with nc.tools, nc.tool.find, nc.tool.get, nc.tool.action, nc.param.object, nc.param.textEdits and nc.op.create, nc.op.update, nc.op.delete, and a lock with bad tools is refused with toolsRefused

- Decided by: Carsten, 2026-11-24, on RFC 0017's open questions, through pm (NOC-504)
- Why: the names are short and each says what it holds.
- Instead of: one namespace nc.agent (nc.agent.tools, nc.agent.create, ...), whose longer names read as if nc.agent.create created an agent.
- Topics: mcp, schema, public-api, rfc-0017
