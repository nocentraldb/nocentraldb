# 2026-11-16 · NOC-323 · TypeScript texts stay in one place until T3

- Decided by: pm, NOC-323, on the coding agent's proposal: TS texts stay in one place until T3
- Why: every developer-facing text lives in texts/texts.json (NOC-329), but texts.json has no kind for the TypeScript SDK yet and the gate does not read packages/sdk; that design is NOC-329's slice T3. `lockOutOfDate`'s message, `model.lock.json is out of date: {why}. Run ncdb model build.`, and its five reasons stay together in `lockOutOfDate` in packages/sdk/src/lockCheck.ts, and the pull request lists them for T3, as NOC-316's slice 2 listed its text.
- Instead of: adding the entries to texts.json now, which needs T3's design for TypeScript (a kind, a generated module and the gate's reading of packages/sdk) in this change first.
- Topics: sdk-ts, texts
