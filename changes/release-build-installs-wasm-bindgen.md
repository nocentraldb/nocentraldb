---
type: fix
tickets: [NOC-407]
introduced: cycle
components: [sdk-ts]
---
## Symptom

The release workflow failed in its `build` job for every beta and release, in the dry run and on the tag, with `target/tools/bin/wasm-bindgen is missing: run moon run repo:wasm-runner`. No beta or release could be published.

## When

On every beta or release tag, and on every dry run that names one. Alphas were not affected: their job installs wasm-bindgen.

## Cause

The `build` job builds the npm packages, and the TypeScript SDK's build runs wasm-bindgen from `target/tools`. Only the jobs `alpha-npm` and `publish-npm` installed it; `build` did not. No beta had been tagged before, so the job had never run.

## Fix

The `build` job installs wasm-bindgen with `moon run repo:wasm-runner` before it builds the npm packages, as the other two jobs do. The release-workflow tests check that every job that builds the npm packages installs wasm-bindgen first.
