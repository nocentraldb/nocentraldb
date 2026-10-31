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

Does your change ship to a user? Add a fragment under `changes/`, see [Writing a change fragment](docs/release-management.md#writing-a-change-fragment) and the writer's guide [changes/README.md](changes/README.md). `pnpm gates` checks every fragment against the schema.

`pnpm gates` runs every check uncached. `pnpm gates:dev` is faster during development but replays cached results, so it does not replace `pnpm gates`. See [Gates](README.md#gates) for the individual checks and how to re-run a single test.
