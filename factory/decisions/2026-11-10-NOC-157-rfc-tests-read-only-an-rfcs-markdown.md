# 2026-11-10 · NOC-157 · An RFC's diagrams carry its number, and the RFC tests read only its Markdown file

- Decided by: coding agent (NOC-157)
- Why: a diagram is named after the RFC it belongs to, as `0001-*.svg` already were, so `docs/rfc` holds several files that start with an RFC's number. Two tests took the first such file as the RFC and read an SVG; they now take the `.md` file, as `rfc-readme.test.mjs` already did.
- Instead of: naming the diagrams without the RFC's number, which would break the convention RFC 0001's diagrams set.
- Topics: rfc, docs, diagrams, tests
