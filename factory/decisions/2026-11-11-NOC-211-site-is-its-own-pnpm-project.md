# 2026-11-11 · NOC-211 · `site/` is its own pnpm project, outside the repository's workspace

- Decided by: coding agent (NOC-211)
- Why: the draft must not change anything outside `site/`. With its own `pnpm-workspace.yaml` and lockfile, Astro, Starlight and their dependencies never enter the root lockfile, and the gates do not build the site until a later ticket wires it into them and CI. `site/biome.json` extends the root configuration, leaving out the built output, the screenshots and the brand files copied unchanged.
- Instead of: adding `site` to the root workspace, which changes the root lockfile and puts the site's build into every gate run now.
- Topics: site, tooling
