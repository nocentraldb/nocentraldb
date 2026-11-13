# 2026-11-13 · NOC-297 · The browser example app builds with tsc and an import map, and its Chromium test runs in browser-view

- Decided by: pm, NOC-297, on the coding agent's proposal
- Why: the example should show what a developer needs to use `@nocentraldb/sdk` in a browser and nothing else. So `apps/tasks-ts` adds no dependency. Its TypeScript is compiled with the repository's `tsc`. The page maps `@nocentraldb/sdk` to the package's build output with an import map, and a small Node server serves it on localhost, where OPFS is available. The type check reads the SDK's source through `paths`, so `pnpm gates` needs no wasm build; the build reads the package's build output. Its Chromium test runs in `browser-view.yml` on app and SDK changes, since an example nobody runs rots. The Cargo workspace takes `apps/*`, so `Cargo.toml` excludes `apps/tasks-ts`, which has no crate.
- Instead of: a bundler or dev server such as Vite, a new dependency the example does not need; or a test that only runs by hand.
- Topics: example-app, sdk-ts, browser, ci
