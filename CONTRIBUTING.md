# Contributing to NoCentralDB

## Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as below, without any additional terms or conditions:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The license expression in every manifest is `MIT OR Apache-2.0`.

## Before a pull request

Set up the tools as described in the [README](README.md#prerequisites), then run the project gate and make sure it is green:

```sh
pnpm install
pnpm gates
```

Does your change ship to a user? Add a fragment under `changes/`, see [Writing a change fragment](docs/release-management.md#writing-a-change-fragment) and the writer's guide [changes/README.md](changes/README.md). `pnpm gates` checks every fragment against the schema, and on the pull request the `changes` check fails when a path outside the allowlist changed and no fragment was added; it names each such path.

The label `skip-changes` passes the `changes` check without a fragment. It is legitimate only when the pull request changes product paths but ships nothing a user would notice: a version raise or a lockfile refresh. Anything a user could notice (a feature, a fix, a changed dependency that alters behaviour, a deprecation) needs a fragment, with `type: internal` when it is not worth a line in the curated notes. The label never waives a detected breaking change. Adding or removing the label re-runs the check.

`pnpm gates` runs every check uncached. `pnpm gates:dev` is faster during development but replays cached results, so it does not replace `pnpm gates`. See [Gates](README.md#gates) for the individual checks and how to re-run a single test.
