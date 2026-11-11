# 2026-11-11 · NOC-211 · The docs have no search until the site is built in CI

- Decided by: coding agent (NOC-211)
- Why: the draft commits its built output, `site/dist`, so that pm can publish a preview. Pagefind, Starlight's search, writes a script of 1,074 lines into it, and the file-size gate fails on any committed code file over 700 lines. Search is off until the site is built in CI instead of committed; a follow-up ticket brings it back.
- Instead of: committing Pagefind's output and weakening the file-size gate for `site/dist`, which GENESIS.md forbids.
- What this does not do: the draft docs cannot be searched; the sidebar and llms.txt remain.
- Topics: site, tooling
