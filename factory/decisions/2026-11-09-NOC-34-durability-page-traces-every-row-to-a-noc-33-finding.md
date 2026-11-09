# 2026-11-09 · NOC-34 · The durability page traces every row to a NOC-33 finding, and the gate checks it

- Decided by: coding agent (NOC-34)
- Why: the ticket asks that every statement trace to a NOC-33 row as measured, documented or unknown. Each table on `docs/browser-durability.md` has a Source column of `kind: Browser · Behaviour` references; `scripts/browser-durability.test.mjs` fails when a reference names no row of the findings table in `docs/browser-storage.md`, names it with another kind, or when a row resting only on unknown findings does not say "unknown". Two MDN statements the page needs (eviction deletes all of an origin's data at once; private browsing data is usually deleted when the session ends) became documented rows of the findings table, so they trace like the rest.
- Instead of: footnotes or links a reviewer checks by eye, which drift when NOC-33's table changes.
- Topics: browser, durability, docs, testing
