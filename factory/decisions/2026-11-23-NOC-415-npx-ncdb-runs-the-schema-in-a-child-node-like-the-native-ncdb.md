# 2026-11-23 · NOC-415 · `npx ncdb` runs the schema in a child Node.js process, as the native `ncdb` does, with the Node.js that runs the bin

- Decided by: coding agent (NOC-415), for the owner's review on the PR
- Why: `crates/ncdb` reads, writes and runs Node.js only through its `Host` trait, and the bin's host spawns `process.execPath` with the same script and arguments the native `ncdb` gives `node`. So the version check, a schema that throws or prints, and every message are the same both ways, and `packages/sdk/test/ncdb.test.ts` compares both on the same projects. `process.execPath` is the `node` on PATH whenever `npx` or the shebang starts the bin.
- Instead of: importing the schema inside the bin's own process, as the NOC-443 proposal sketched, which saves one process start but would give the schema module's output and failures a second path that words them differently from the native `ncdb`.
- Topics: cli, sdk-ts, node
