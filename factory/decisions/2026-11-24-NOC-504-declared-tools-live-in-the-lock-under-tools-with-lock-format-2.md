# 2026-11-24 · NOC-504 · Declared tools live in model.lock.json under tools, and a lock that declares tools has format 2

- Decided by: Carsten, 2026-11-24, on RFC 0017's open questions, through pm (NOC-504)
- Why: one file to build, check in CI and read; an SDK that reads only format 1 refuses such a lock openly instead of ignoring what it cannot read, and asking for a current SDK costs little in the beta.
- Instead of: a separate model.tools.json beside the lock, which can drift from it; or tools in a format-1 lock, which needs every format-1 reader to ignore an unknown key, a promise format 1 never made.
- Topics: mcp, schema, lock, rfc-0017
