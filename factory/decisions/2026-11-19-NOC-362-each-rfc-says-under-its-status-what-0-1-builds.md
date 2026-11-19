# 2026-11-19 · NOC-362 · Each RFC says under its status what 0.1 builds

- Decided by: pm, NOC-362, on the coding agent's proposal
- Why: a reader of a decided RFC could not tell the design from what release 0.1 ships. Each RFC now carries one line right under its status, "**At 0.1.**", which says built, partly built or not built and names what is not built; `scripts/rfc-at-0-1.test.mjs` requires it. The status line stays as the owner's decision left it, since the RFC tests pin it, and the design text stays a design: a passage describes what is not built yet without a "not yet" of its own, unless it claimed a state of the code that is no longer true. RFC 0003's and RFC 0007's own sentences on what is built moved into the line.
- Instead of: changing each status line, which records the owner's decision and not the code; marking "not built" in every passage, which would scatter the release's state over thousands of lines; a separate page per release, which a reader of one RFC would not find.
- Topics: rfc, docs, release
