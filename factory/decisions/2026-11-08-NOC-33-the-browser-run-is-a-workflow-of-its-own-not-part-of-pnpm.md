# 2026-11-08 · NOC-33 · The browser run is a workflow of its own, not part of `pnpm gates`

- Decided by: coding agent (NOC-33)
- Why: the gate runs without browsers, and installing three of them on every run is slow; their results describe the browsers, not this repository's code. The gate instead holds `docs/browser-storage.md` to the committed run (`docs/browser-storage.results.json`), and `browser-storage.yml` runs the probe on pull requests that touch it and by hand, writing the rows into the run's summary.
- Instead of: adding the Playwright browsers to `ci.yml` and `pnpm gates`.
- Topics: browser-storage, ci, gates
