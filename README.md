# NoCentralDB

NoCentralDB is a local-first database designed for teams of people and agents: every member, human or agent, works on a full local copy of the spaces they belong to, under their own keys, and a relay that holds no keys carries the encrypted changes between them. The design is in [RFC 0001](docs/rfc/0001-nocentraldb.md). Nothing is released yet; this repository currently holds the monorepo foundation and placeholder projects.

## Layout

```
crates/ncdb-core/     Rust core (placeholder)
packages/sdk/         @nocentraldb/sdk, the TypeScript SDK (placeholder)
apps/                 applications (empty for now)
docs/rfc/             design documents
scripts/              repository tooling (check-pins.mjs)
.moon/                moon workspace, toolchains and the tasks every project inherits
moon.yml              the root project `repo`: gates the root config files
```

Rust crates form a Cargo workspace (`Cargo.toml`, `crates/*`); TypeScript projects form a pnpm workspace (`pnpm-workspace.yaml`, `apps/*`, `packages/*`). moon runs the tasks of both.

## Prerequisites

- [proto](https://moonrepo.dev/proto) installs the pinned moon, Node and pnpm from `.prototools`.
- [rustup](https://rustup.rs) installs the pinned Rust from `rust-toolchain.toml`, with `rustfmt` and `clippy`. The gate does this itself in the task `repo:rust-toolchain` (`rustup toolchain install --no-self-update`), which every Rust task waits for: rustup has no install lock, so parallel Cargo tasks that each auto-install a missing toolchain break each other.

Every tool version has exactly one source: `.prototools` for moon, Node and pnpm, `rust-toolchain.toml` for Rust. proto reads `.prototools` only (`detect-strategy = "only-prototools"`) and leaves Rust to rustup: when proto installed the toolchain from `rust-toolchain.toml` it came out without `cargo`. `.nvmrc` and the `packageManager` field in `package.json` repeat the Node and pnpm pins, and `scripts/check-pins.mjs` fails the gate when they differ. Node 24 is the minimum; `pnpm install` refuses an older Node.

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

**The project gate is `pnpm gates`.** It runs every gate task of every project through moon, uncached (`moon exec :gates --force --on-failure continue --summary normal`): every task is forced, no cached result is read, and a failing task does not stop the others, so every gate has a result. The closing summary lists every task as `pass`, `fail` or `skip`; it exits non-zero if any task failed.

`pnpm gates:dev` (`moon run :gates`) is the cached inner loop for development: moon skips tasks whose inputs are unchanged and replays their cached result. It never counts as verification.

What each project's `gates` task runs:

| Project | Tasks |
| --- | --- |
| Rust crates (`ncdb-core`) | `format-check` (`cargo fmt --check`), `lint` (`cargo clippy --locked --all-targets -- -D warnings`), `test` (`cargo test --locked`) |
| TypeScript projects (`sdk`) | `format-check` and `lint` (`biome check --error-on-warnings .`), `typecheck` (`tsc --noEmit`), `test` (`vitest run`) |
| Root project (`repo`) | `format-check` (Biome on the root JSON/YAML config, `moon.yml`, `.moon/` and `scripts/`), `pins` (`node scripts/check-pins.mjs`); `rust-toolchain` runs before every Rust task |

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
```

Re-running one test in isolation, e.g. to check a suspected flake:

```sh
cargo test --locked -p ncdb-core <test name>
pnpm --filter @nocentraldb/sdk exec vitest run -t "<test name>"
```

### Caching and what moon buys

moon gives the repository one task graph over Rust and TypeScript, runs independent tasks in parallel, and caches task results for the inner loop (`pnpm gates:dev`). Independent checks (`pnpm gates`, CI) do not trust moon's task cache, but they still reuse the self-validating caches of the compilers and package manager (Cargo's `target/`, the pnpm store), so compilation stays incremental there too.

For the inner loop, every task also takes the root files it depends on as inputs: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` for Rust tasks; `biome.json`, `tsconfig.base.json`, `package.json`, `pnpm-lock.yaml`, `.prototools` for TypeScript tasks. Every Rust task takes the sources of ALL crates as inputs: coarse, but a change in one crate re-runs the checks of every crate that may depend on it. Tracking the installed Rust toolchain beyond `rust-toolchain.toml` is best effort; a stale inner-loop result is caught by `pnpm gates`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option. See [CONTRIBUTING.md](CONTRIBUTING.md).
