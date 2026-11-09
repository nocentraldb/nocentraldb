# 2026-11-09 · NOC-70 · Biome does not lint the generated API report

- Decided by: pm (option A), on NOC-70; the owner reviews it with the PR
- Why: `packages/sdk/etc/api-report.d.ts` is tsc's declaration output, copied verbatim by `scripts/release/api-report.mjs`: four-space indentation, an `export {};` when a file has non-exported types, no blank line after `export * as nc`. Biome's format and lint rules fail on it as soon as the SDK has more than a one-line API, although no person writes the file. `biome.json` excludes this one path; `repo:api-report` still holds its bytes exactly to what tsc generates.
- Instead of: a separate tooling PR first (B), or having the api-report script format the report with Biome before comparing it (C), which changes the release tooling.
- Topics: gates, biome, sdk, api-report
