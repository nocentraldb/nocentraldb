# 2026-11-23 · NOC-485 · The MCP server serves the tools an app declares in its model, and keeps the generic tools behind a developer flag

- Decided by: Carsten, 2026-11-23, on RFC 0017's questions, through pm (NOC-488)
- Why: the app knows its domain and a model uses its named tools more reliably than generic ones; NoCentralDB keeps what every app would otherwise rebuild: the member, its key, pairing, the copy and its sync, the host and the transports. Declared tools are data that `ncdb model build` checks, so one binary serves every app, TypeScript apps included.
- Instead of: generic tools only (A), a Rust crate each app builds its own server on (B), or nothing for now (D).
- Topics: mcp, agents, schema, rfc-0017
