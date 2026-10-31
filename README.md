# NoCentralDB

NoCentralDB is a local-first database designed for teams of people and agents: every member, human or agent, works on a full local copy of the spaces they belong to, under their own keys, and a relay that holds no keys carries the encrypted changes between them. The design is in [RFC 0001](docs/rfc/0001-nocentraldb.md). Nothing is released yet; this repository currently holds the monorepo foundation and placeholder projects.

## Layout

```
crates/ncdb-core/     Rust core (placeholder)
packages/sdk/         @nocentraldb/sdk, the TypeScript SDK (placeholder)
apps/                 applications (empty for now)
docs/rfc/             design documents
scripts/              repository tooling (check-pins.mjs) and its tests, incl. ci-workflow.test.mjs (the CI contract)
.moon/                moon workspace, toolchains and the tasks every project inherits
moon.yml              the root project `repo`: gates the root config files
```

Rust crates form a Cargo workspace (`Cargo.toml`, `crates/*`); TypeScript projects form a pnpm workspace (`pnpm-workspace.yaml`, `apps/*`, `packages/*`). moon runs the tasks of both.

## Prerequisites

- [proto](https://moonrepo.dev/proto) installs the pinned moon, Node and pnpm from `.prototools`.
- [rustup](https://rustup.rs) installs the pinned Rust from `rust-toolchain.toml`, with `rustfmt` and `clippy`. The gate does this itself in the task `repo:rust-toolchain` (`rustup toolchain install --no-self-update`), which every Rust task waits for: rustup has no install lock, so parallel Cargo tasks that each auto-install a missing toolchain break each other.

Every tool version has exactly one source: `.prototools` for moon, Node and pnpm, `rust-toolchain.toml` for Rust. proto reads `.prototools` only (`detect-strategy = "only-prototools"`) and leaves Rust to rustup: when proto installed the toolchain from `rust-toolchain.toml` it came out without `cargo`. `.nvmrc` and the `packageManager` field in `package.json` repeat the Node and pnpm pins, and `scripts/check-pins.mjs` fails the gate when they differ. The same script holds npm dependencies to one exact version each: every dependency field of every `package.json`, `pnpm.overrides` and `resolutions` in the root `package.json`, and `overrides`, `catalog` and `catalogs` in `pnpm-workspace.yaml`; a `catalog:` reference must name an entry of its catalog. It uses Node built-ins only, so it reads `pnpm-workspace.yaml` with a narrow parser: those three sections must be block mappings of plain or quoted scalars, and anything else in them (flow collections, anchors, aliases, tags, block scalars) fails the gate rather than being skipped. Node 24 is the minimum; `pnpm install` refuses an older Node.

From a fresh clone:

```sh
proto use        # installs the pinned moon, Node and pnpm
pnpm install
pnpm gates
```

### Getting the tools on PATH

`proto use` puts moon, Node and pnpm on PATH through the proto shims. The shims directory must come before any other Node manager (nvm in particular) on PATH, in every shell that runs the gates, including non-interactive ones (scripts, agents, CI) that never read an interactive shell profile. Set it where every shell reads it, e.g. `~/.zshenv` for zsh:

```sh
export PROTO_HOME="$HOME/.proto"
export PATH="$PROTO_HOME/shims:$PROTO_HOME/bin:$PATH"
```

Check with `which node pnpm moon`: each must resolve to `~/.proto/shims/…`. Inside the repository the proto shims run the Node pinned in `.prototools`, even when another Node comes earlier on PATH; `devEngines.runtime` in `package.json` rejects Node < 24 wherever pnpm runs without proto (`ERR_PNPM_BAD_RUNTIME_VERSION`).

## Gates

**The project gate is `pnpm gates`.** It runs every gate task of every project through moon, uncached (`moon exec :gates --force --base HEAD --head HEAD --on-failure continue --summary normal`): every task is forced, no cached result is read, and a failing task does not stop the others, so every gate has a result. `--base HEAD --head HEAD` only keeps moon's changed-file query (which `moon exec` runs even when forced) resolvable in a shallow CI checkout; the gate selects no task by changed files. The closing summary lists every task as `pass`, `fail` or `skip`; it exits non-zero if any task failed.

CI (`.github/workflows/ci.yml`) runs `pnpm gates` on pull requests and on pushes to `main`, as the check named `gates`.

The job times out after 30 minutes. A new push to a pull request cancels that pull request's superseded run; runs on `main` are never cancelled, so every commit on `main` gets a completed `gates` run. The Cargo cache (registry and `target/`) is keyed exactly on `rust-toolchain.toml` and `Cargo.lock`, with no fallback key: an entry is saved once and never grows, and a toolchain or lockfile change starts a cold build.

Every action in the workflow is pinned to a full commit SHA, with its release as a comment (`uses: actions/checkout@<sha> # v7.0.1`). Dependabot (`.github/dependabot.yml`, the `github-actions` ecosystem, weekly) keeps those pins current: it resolves each SHA together with its version comment and opens a pull request that moves both to the new release, which then has to pass `gates` like any other change. Dependabot watches the actions only. npm is left out because its updates would bypass the single-source pins (`.prototools` for pnpm and Node, `packageManager` and `.nvmrc` repeating them, checked by `scripts/check-pins.mjs`) and would regenerate `pnpm-lock.yaml` with Dependabot's own pnpm, not the pinned one; cargo is left out because the workspace has no third-party crates yet, so there is nothing to update. Revisit both when that changes.

`scripts/ci-workflow.test.mjs` holds the workflow to these rules: it parses `ci.yml`, `package.json`, `pnpm-workspace.yaml` and `.github/dependabot.yml` and asserts the triggers, `contents: read`, the job id `gates`, the SHA-pinned actions, the exact `pnpm gates` command (`--on-failure continue` included), the default checkout depth, the 30-minute timeout, the concurrency block, both cache keys with no Cargo fallback key, the empty `onlyBuiltDependencies` allow-list, and the Dependabot configuration (`github-actions` weekly, nothing else). Each check also runs against mutated copies of those files and must reject every one, so a check that cannot fail turns the gate red. A deliberate change to any of these rules changes the test with it.

`pnpm gates:dev` (`moon run :gates`) is the cached inner loop for development: moon skips tasks whose inputs are unchanged and replays their cached result. It never counts as verification.

What each project's `gates` task runs:

| Project | Tasks |
| --- | --- |
| Rust crates (`ncdb-core`) | `format-check` (`cargo fmt --check`), `lint` (`cargo clippy --locked --all-targets -- -D warnings`), `test` (`cargo test --locked`) |
| TypeScript projects (`sdk`) | `format-check` and `lint` (`biome check --error-on-warnings .`), `typecheck` (`tsc --noEmit`), `test` (`vitest run`) |
| Root project (`repo`) | `format-check` (Biome on the root JSON/YAML config, `moon.yml`, `.moon/` and `scripts/`), `pins` (`node scripts/check-pins.mjs`), `test` (`node --test scripts/*.test.mjs`: the tests of check-pins, and `ci-workflow.test.mjs`, which parses `ci.yml`, the `gates` script, `pnpm-workspace.yaml` and `.github/dependabot.yml` and asserts the CI contract described above); `rust-toolchain` runs before every Rust task |

Any formatting difference, clippy warning, Biome diagnostic (warnings included), type error or failing test fails the gate. Cargo commands that resolve dependencies run with `--locked`, so a `Cargo.toml` change without the matching `Cargo.lock` fails.

Running one gate on its own, uncached:

```sh
moon run ncdb-core:format-check --force
moon run ncdb-core:lint --force
moon run ncdb-core:test --force
moon run sdk:format-check --force
moon run sdk:lint --force
moon run sdk:typecheck --force
moon run sdk:test --force
moon run repo:format-check --force
moon run repo:pins --force
moon run repo:test --force
```

Re-running one test in isolation, e.g. to check a suspected flake:

```sh
cargo test --locked -p ncdb-core <test name>
pnpm --filter @nocentraldb/sdk exec vitest run -t "<test name>"
node --test --test-name-pattern "<test name>" scripts/check-pins.test.mjs
node --test --test-name-pattern "<test name>" scripts/ci-workflow.test.mjs
```

### Caching and what moon buys

moon gives the repository one task graph over Rust and TypeScript, runs independent tasks in parallel, and caches task results for the inner loop (`pnpm gates:dev`). Independent checks (`pnpm gates`, CI) do not trust moon's task cache, but they still reuse the self-validating caches of the compilers and package manager (Cargo's `target/`, the pnpm store), so compilation stays incremental there too.

For the inner loop, every task also takes the root files it depends on as inputs: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` for Rust tasks; `biome.json`, `tsconfig.base.json`, `package.json`, `pnpm-lock.yaml`, `.prototools` for TypeScript tasks. Every Rust task takes the sources of ALL crates as inputs: coarse, but a change in one crate re-runs the checks of every crate that may depend on it. Tracking the installed Rust toolchain beyond `rust-toolchain.toml` is best effort; a stale inner-loop result is caught by `pnpm gates`.

## Package names

The project holds these names on the public registries. The unscoped ones carry a placeholder release `0.0.0` that contains no code and points back to this repository.

| Registry | Name | Status |
| --- | --- | --- |
| npm | `@nocentraldb/*` | npm organisation `nocentraldb`; the SDK will be published as `@nocentraldb/sdk` |
| npm | `nocentraldb` | placeholder |
| npm | `ncdb` | not publishable: npm rejects it as too similar to existing packages, so nobody else can take it either |
| crates.io | `nocentraldb` | placeholder |
| crates.io | `ncdb-core` | placeholder; the core crate in `crates/ncdb-core` takes it over with its first real release |
| crates.io | `ncdb` | placeholder |

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [CONTRIBUTING.md](CONTRIBUTING.md).
