# The repository

How this repository is laid out, how to set up its tools, and the gates every change passes. The [README](../README.md) says what NoCentralDB is; [CONTRIBUTING.md](../CONTRIBUTING.md) says what a contribution needs. [Merge hot spots](merge-hot-spots.md) lists the files many pull requests change at once, and why each stays.

## Layout

```
crates/ncdb-core/     the Rust core: changes, keys, the fold into the SQLite view and the merge rules, natively and as WebAssembly
crates/ncdb-plugin/   what a plugin field type implements and depends on (RFC 0011): the FieldType trait, Data, the ids of an operation, Plugin
crates/ncdb-harness/  the convergence harness (RFC 0011): RFC 0004's checks C1 to C7 for a field type, built in or plugin
crates/ncdb-test-counter/  a plugin for the tests, test/counter (RFC 0011); not published
crates/ncdb/          the command line ncdb: ncdb model build and ncdb model check (RFC 0005)
crates/ncdb-wasm/     the command line ncdb compiled to WebAssembly, the core of @nocentraldb/sdk's ncdb bin; not published
crates/ncdb-relay/    the relay core: RFC 0007's checks, the backend interface, its conformance suite and the SQLite backend
crates/ncdb-relay-server/  the relay server: RFC 0007's WebSocket protocol over the relay core, the ncdb-relay binary that runs it locally on SQLite, and the test relay an app starts in its own tests (local::TestRelay)
crates/ncdb-embedder-example/  an app's own tests against the test relay, through the public SDK alone (NOC-507); not published
crates/ncdb-relay-aws/     the relay's AWS backend on DynamoDB and S3, and the check of what the AWS Emulator covers (docs/aws-emulator.md)
crates/ncdb-relay-lambda/  the relay as one Lambda function behind API Gateway's WebSocket API, on the AWS backend, and a local stand-in for API Gateway to test it
infra/aws/            the relay's AWS infrastructure as a CDK app in TypeScript (RFC 0007, "The AWS backend"); its tests synthesize it with no AWS credentials
crates/nocentraldb/   the Rust SDK (RFC 0009): open a database, write, read, query and subscribe, sync through a relay, accounts and erasure
packages/sdk/         @nocentraldb/sdk, the TypeScript SDK in the browser (RFC 0009), its core in a worker as WebAssembly, and the ncdb bin (npx ncdb); etc/api-report.d.ts is its committed API report
apps/tasks-rust/       the example app on the Rust SDK: a task list on two devices, synced through a local relay
apps/tasks-ts/         the example app on the TypeScript SDK: a task list in the browser, its lists subscriptions; its Chromium test runs in browser-view.yml
docs/rfc/             design documents; README.md lists them and says where feedback goes
docs/repository.md          this page: the layout, the tools and the gates
docs/getting-started.md     the getting-started guide for the Rust SDK; the doc tests of `nocentraldb` compile its Rust snippets
docs/getting-started-browser.md  the getting-started guide for the TypeScript SDK in the browser; scripts/doc-snippets.test.mjs type-checks its snippets
docs/browser-durability.md  browser apps and durability: what a browser copy keeps, what is lost when, and what to tell users
docs/use-cases.md, faq.md, glossary.md, browser-storage.md  more developer docs; the site renders these and the guides (site/src/repo-docs.mjs)
docs/release-management.md  how releases are made
docs/release-flow.svg       the release flow diagram
factory/              the software factory: GENESIS.md (the product), FACTORY.md (how agents build it), CONFIG.md, decisions/ (the decision log, read with pnpm decisions)
changes/              change fragments, one per product-changing pull request (writer's guide: changes/README.md)
release-notes/        curated release notes, one X.Y.Z.md per release, and index.json: every published beta and release
scripts/              repository tooling (check-pins.mjs) and its tests, incl. the workflow contracts ci-workflow.test.mjs, changes-workflow.test.mjs and release-workflow*.test.mjs, and contributor-kit.test.mjs
scripts/lib/          shared test helpers (workflow-checks.mjs: the workflow contracts' mutant runner) and test-changes/, the reader behind check-test-changes.mjs
scripts/release/      release tooling (check-fragments.mjs, check-version.mjs, set-version.mjs, next-version.mjs, render-notes.mjs, verify-tag.mjs, index.mjs, shipped-fragments.mjs, publishable.mjs, commit-back.mjs, api-report.mjs; the PM's draft-notes.mjs, cut.mjs, promotion-check.mjs; lib/: cli, fragments, git, glob, manifests, semver, test-repo) and its tests
.github/workflows/    ci.yml (the `gates` check), changes.yml (the `changes` check) and release.yml (alphas after a green `ci` on `main`)
.github/              the contributor kit: issue forms, the RFC feedback discussion form, the pull request template, AI_POLICY.md, SECURITY.md, CODE_OF_CONDUCT.md
.moon/                moon workspace, toolchains and the tasks every project inherits
moon.yml              the root project `repo`: gates the root config files
```

Rust crates form a Cargo workspace (`Cargo.toml`, `crates/*` and `apps/tasks-rust`); TypeScript projects form a pnpm workspace (`pnpm-workspace.yaml`, `apps/*`, `packages/*`). moon runs the tasks of both.

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

CI (`.github/workflows/ci.yml`) runs `pnpm gates` on pull requests, on each commit of the merge queue ([The merge queue](#the-merge-queue)) and on pushes to `main`, as the check named `gates`.

The job times out after 45 minutes. A new push to a pull request cancels that pull request's superseded run; runs on `main` and in the merge queue are never cancelled, so every commit on `main` and every queue commit gets a completed `gates` run. The Cargo cache (registry and `target/`) is keyed exactly on `rust-toolchain.toml` and `Cargo.lock`, with no fallback key: an entry is saved once and never grows, and a toolchain or lockfile change starts a cold build.

The proto toolchain is cached the same way, in every workflow job that installs it (`ci.yml`, `changes.yml`, and each job of `release.yml`): `~/.proto` (proto, moon, Node, pnpm, and proto's plugins) is restored under one exact key, the hash of `.prototools` and `scripts/install-toolchain.sh`, with no fallback key. The script pins proto's own version (`.moon/toolchains.yml` pins moon's proto to the same one). With a warm cache it downloads nothing; on a cold cache it downloads proto from its GitHub release and runs `proto install`, retrying each up to three times, 10 s and 20 s apart, and then fails with an error that names the host it could not reach. `scripts/toolchain-workflow.test.mjs` holds the workflows to this and runs the script against fake `curl` and `proto`.

Every action in the workflow is pinned to a full commit SHA, with its release as a comment (`uses: actions/checkout@<sha> # v7.0.1`). Dependabot (`.github/dependabot.yml`, the `github-actions` ecosystem, weekly) keeps those pins current: it resolves each SHA together with its version comment and opens a pull request that moves both to the new release, which then has to pass `gates` like any other change. Dependabot watches the actions only. npm is left out because its updates would bypass the single-source pins (`.prototools` for pnpm and Node, `packageManager` and `.nvmrc` repeating them, checked by `scripts/check-pins.mjs`) and would regenerate `pnpm-lock.yaml` with Dependabot's own pnpm, not the pinned one; cargo is left out because the workspace has no third-party crates yet, so there is nothing to update. Revisit both when that changes.

`scripts/ci-workflow.test.mjs` holds the workflow to these rules: it parses `ci.yml`, `package.json`, `pnpm-workspace.yaml` and `.github/dependabot.yml` and asserts the triggers, `contents: read`, the job id `gates`, the SHA-pinned actions, the exact `pnpm gates` command (`--on-failure continue` included), the default checkout depth, the 45-minute timeout, the concurrency block, both cache keys with no Cargo fallback key, the empty `onlyBuiltDependencies` allow-list, and the Dependabot configuration (`github-actions` weekly, nothing else). Each check also runs against mutated copies of those files and must reject every one, so a check that cannot fail turns the gate red. A deliberate change to any of these rules changes the test with it.

A second check, `changes` (`.github/workflows/changes.yml`), runs on every pull request, again when a label is added or removed, and on each pull request's commit in the merge queue. A pull request that changes a path outside the allowlist (`ALLOWLIST` in `scripts/release/check-fragments.mjs`, listed in [docs/release-management.md](release-management.md#writing-a-change-fragment): docs, `scripts/`, `.github/`, moon config, tests and similar) must add a change fragment under `changes/`; otherwise the check fails and names each such path. The label `skip-changes` waives that rule for a pull request that changes product paths but ships nothing to a user (a version raise, a lockfile refresh); it never waives the validation of the fragments the pull request adds or modifies. The check runs on the pull request's merge ref, the pull request merged into `main` as `main` stands when the job runs, and compares everything with that merge commit's first parent, `HEAD^1`: a change `main` gained after the pull request's last push is in the baseline and never counted against the pull request. It first checks that `HEAD`'s second parent is the pull request's head, and fails otherwise. In the merge queue (`merge_group`) the checkout is the queue's commit, the pull request squashed onto `main` with the pull requests ahead of it, and the check compares with its first parent, `HEAD^1`, the same way; it first checks that `HEAD` is the queue's commit. The event names no pull request, so a step reads its number from the queue's branch (`gh-readonly-queue/main/pr-<number>-<sha>`), fetches its head (`refs/pull/<number>/head`) and reads its `skip-changes` label through the API, which is why the workflow also grants `pull-requests: read`. The same check detects breaking changes: it checks `HEAD^1` out as a worktree, runs `cargo-semver-checks` (through `obi1kenobi/cargo-semver-checks-action`, on the toolchain `rust-toolchain.toml` pins) against it, and diffs the TypeScript SDK's API report against the same commit with `node scripts/release/api-report.mjs --base HEAD^1`. When either reports a breaking change, the pull request must add a fragment of type `breaking`, and `skip-changes` does not waive that. Its last step is the test-change check (NOC-508): a pull request that removes a test, skips one (`#[ignore]`, `.skip`, `.only`, a `skip` option), removes an assertion or changes an expected value in one fails unless one of its own commits carries a `Test-change: <reason>` trailer that names the reason in words; it runs after a failed step too, and no label waives it (`node scripts/check-test-changes.mjs --base <base sha> --head <head sha>` reproduces it). In the merge queue it compares `HEAD^1` with `HEAD` and reads the trailers from the pull request's own commits (`--pr-head <its head>`), since the squash commit's message does not keep them as trailers. The check reads only (`contents: read`, `pull-requests: read`), fetches the full history, and runs `node scripts/release/check-fragments.mjs --base HEAD^1 --head HEAD [--skip] [--require-breaking]`; locally, `--base origin/main --head HEAD` reproduces it. `scripts/changes-workflow.test.mjs` holds the workflow to that contract (triggers incl. `labeled`/`unlabeled`, permissions, the job id `changes`, SHA pins, full history, the exact command and its environment, the baseline worktree at `HEAD^1` after the merge-ref check, the semver step that may fail without failing the job, the API report's exit codes, both feeding `--require-breaking` before the fragment step, the test-change step last, concurrency, a 15-minute timeout), with mutants like the CI contract; both share the runner in `scripts/lib/workflow-checks.mjs`. `scripts/changes-workflow-merge-ref.test.mjs` runs the baseline, API report and fragment steps against a fixture merge ref whose `main` moved on after the pull request's push, and `scripts/changes-workflow-merge-queue.test.mjs` runs them, with the step that finds the pull request and the test-change step, against a fixture queue commit; both share `scripts/lib/changes-fixture.mjs`.

A third workflow, `release` (`.github/workflows/release.yml`), is not a pull-request check. Its job `alpha` runs after every completed `ci` run on `main` and turns a green run of a push to `main` that added a fragment other than `internal` into an alpha, a tag `vX.Y.Z-alpha.N` and a GitHub prerelease ([Alpha](release-management.md#alpha)); it never runs for a `ci` run that a pull request triggered. Its publish jobs run on a pushed `v*` tag that is not an alpha ([Beta](release-management.md#beta), [Release](release-management.md#release)): `verify` checks the tag with `verify-tag` on the tagged commit and then runs `pnpm gates` on it, `build` writes the tag's version into its checkout and builds, `publish` makes the GitHub release (a prerelease for a beta) and reports on the registry steps, and `commit-back` commits the release index, the notes archive and the removal of the shipped fragments to `main` with `scripts/release/commit-back.mjs`. The workflow is `contents: read`; only `alpha`, `publish` and `commit-back` hold `contents: write`, and none of them leaves its token in the checkout. A `workflow_dispatch` is always a dry run that tags and publishes nothing: `gh workflow run release.yml --ref main` runs the alpha job, and `gh workflow run release.yml --ref release/X.Y -f tag=vX.Y.Z-beta.N` runs `verify` and `build` against that ref and stops. Three files hold it to its contract with mutants and the same runner, sharing the helpers in `scripts/lib/release-workflow.mjs`: `scripts/release-workflow-alpha.test.mjs` for `alpha`, `scripts/release-workflow-publish.test.mjs` for the publish jobs and `scripts/release-workflow.test.mjs` for the triggers, the permissions and what every job shares. Together they check, for `alpha`, the triggers, the job's `if` with each of its five conditions and the tagless dispatch, the head guard as the first step before any checkout and every later step gated on it, the checkout of the run's `head_sha` with full history and tags and without persisted credentials, no tag or release on a dispatch, next-version's skip honoured, a prerelease with the rendered notes whose tag is created in the same API call (no `git tag`/`git push`) and the `release-alpha` concurrency group that never cancels; for the publish jobs, their `if` (no alphas, a dispatch only with a tag), the permissions per job, a dispatch that never reaches `publish` or `commit-back`, `verify-tag` before the gate and both before `build`, the release branches fetched first, `set-version` before the build, the idempotent GitHub release with `--prerelease` or `--latest`, registry steps that only report, the checkout of `main` and `commit-back.mjs` in `commit-back`, and a `release-publish-<ref>` concurrency group that never cancels; for every job, `bash` with `pipefail`, SHA pins and a timeout (20 minutes for `alpha` and `publish`, 45 for `verify` and `build`, 15 for `commit-back`).

`pnpm gates:dev` (`moon run :gates`) is the cached inner loop for development: moon skips tasks whose inputs are unchanged and replays their cached result. It never counts as verification.

**Disk space.** Debug and test builds keep only line tables (`debug = "line-tables-only"` in the workspace `Cargo.toml`), so panics still name file and line while a full `pnpm gates` stays near 15 GiB of `target/` instead of filling a small disk; when space runs out anyway, `cargo clean -p <crate>` frees one crate, `cargo clean --target wasm32-unknown-unknown` frees the wasm builds under `target/wasm32-unknown-unknown/`, and `cargo clean` frees everything (NOC-279).

What each project's `gates` task runs:

| Project | Tasks |
| --- | --- |
| Rust crates (`ncdb-core`) | `format-check` (`cargo fmt --check`), `lint` (`cargo clippy --locked --all-targets -- -D warnings`), `test` (`cargo test --locked`), `build-wasm` (`cargo build --locked --target wasm32-unknown-unknown`: the crate builds for the browser target, so it uses nothing the browser lacks), `test-wasm` (`cargo test --locked --target wasm32-unknown-unknown`: its tests pass as WebAssembly, run in Node by `wasm-bindgen-test-runner`); and, for rich text, which folds only with the compiler cfg `ncdb_rich_text` that only these tasks set, until its guard is complete (NOC-468), `lint-rich-text` and `test-rich-text` (the same `lint` and `test` with `RUSTFLAGS="--cfg ncdb_rich_text"`, in `target/rich-text`) and `test-wasm-rich-text` (the rich-text tests with the cfg on wasm32) |
| Rust plugin crate (`ncdb-plugin`) | `format-check`, `lint`, `test`, `build-wasm` and `test-wasm` as for `ncdb-core`: every plugin runs natively and in the browser, so the crate does too |
| Rust harness (`ncdb-harness`) | `format-check`, `lint`, `test`, `build-wasm` and `test-wasm` as for `ncdb-core`: a plugin's author runs it natively and on wasm32 (C3) |
| Rust test plugin (`ncdb-test-counter`) | `format-check`, `lint` and `test` as for `ncdb-core`; the core's tests run it on wasm32, so it has no wasm tasks of its own |
| Rust command line (`ncdb`) | `format-check`, `lint` and `test` as for `ncdb-core` (its tests run `ncdb model build` and `ncdb model check` on TypeScript schemas, so they need Node.js 24 on PATH); `ncdb-wasm` builds its library for wasm32, so it has no wasm tasks of its own |
| The `ncdb` bin's core (`ncdb-wasm`) | `format-check`, `lint` and `test` as for `ncdb-core`, and `build-wasm`: the crate runs `ncdb` on the host of `@nocentraldb/sdk`'s bin (NOC-415); the SDK's `test` runs what it builds against the native `ncdb` |
| Rust relay core (`ncdb-relay`) | `format-check`, `lint` and `test` as for `ncdb-core` (its tests run the backend conformance suite against SQLite in temporary directories); the relay runs natively only, so it has no wasm tasks |
| Rust relay server (`ncdb-relay-server`) | `format-check`, `lint` and `test` as for `ncdb-core` (its tests start the server, the `ncdb-relay` binary and the test relay, on SQLite in temporary directories or in memory, and talk to it over local WebSocket connections); it runs natively only, so it has no wasm tasks |
| Embedder example (`ncdb-embedder-example`) | `format-check`, `lint` and `test` as for `ncdb-core` (its tests open two databases of the Rust SDK and sync them through the test relay); native only, so it has no wasm tasks |
| Rust relay Lambda function (`ncdb-relay-lambda`) | `format-check`, `lint` and `test` as for `ncdb-core` (its tests drive the handler with recorded API Gateway events, and through the local stand-in for API Gateway over WebSocket, on SQLite in temporary directories); plus `test-sdk-sync`, the Rust SDK's sync suite run through that stand-in (`NCDB_TEST_RELAY=lambda`); native only, so it has no wasm tasks |
| Rust relay AWS backend (`ncdb-relay-aws`) | `format-check`, `lint` and `test` as for `ncdb-core`; its unit tests run as any crate's; its tests against the AWS Emulator (the coverage check, the conformance suite and the backend's own) are ignored unless an emulator runs (`docs/aws-emulator.md`), and the CI job `aws-emulator` runs them; native only, so it has no wasm tasks |
| Rust SDK (`nocentraldb`) | `format-check`, `lint`, `test`, `build-wasm` and `test-wasm` as for `ncdb-core`, except that `test-wasm` runs only the library's in-memory tests (`--lib`): its sync, relay, keystore and reopen tests need sockets, threads, a file system or a keystore and stay native (NOC-175); and `lint-rich-text` and `test-rich-text` as for `ncdb-core`, with the feature `bridge` on and `test-rich-text` on the library's tests only, for the rich-text editor binding's bridge calls (NOC-461) |
| The TypeScript SDK's core (`nocentraldb-wasm`) | `format-check`, `lint`, `test`, `build-wasm` and `test-wasm` as for `nocentraldb`: the crate wraps the Rust SDK's bridge for the worker of `@nocentraldb/sdk` (NOC-176), its tests run natively and in Node on wasm32; and `build-wasm-rich-text` and `lint-rich-text`, the wasm32 build and its lint with the cfg `ncdb_rich_text`, whose core exports a marker the package's build refuses (NOC-461) |
| TypeScript projects (`sdk`, `infra-aws`) | `format-check` and `lint` (`biome check --error-on-warnings .`), `typecheck` (`tsc --noEmit`), `test` (`vitest run`); the SDK's `test` also builds the native `ncdb` with cargo and the `ncdb` bin's core with `wasm-bindgen`, and checks that both say and write the same, so it waits for `rust-toolchain` and `wasm-runner` |
| Root project (`repo`) | `format-check` (Biome on the root JSON/YAML config, `moon.yml`, `.moon/` and `scripts/`), `pins` (`node scripts/check-pins.mjs`), `file-size` (`node scripts/check-file-size.mjs`: no tracked code or test file is longer than the limit in the row "Largest code or test file" of `factory/CONFIG.md`), `test` (`node --test 'scripts/*.test.mjs' 'scripts/release/**/*.test.mjs'`, both patterns quoted so Node expands them itself: the tests of check-pins, `ci-workflow.test.mjs`, which parses `ci.yml`, the `gates` script, `pnpm-workspace.yaml` and `.github/dependabot.yml` and asserts the CI contract described above, `changes-workflow.test.mjs` and the three `release-workflow*.test.mjs`, which assert the contracts of `changes.yml` and `release.yml`, `contributor-kit.test.mjs`, which checks the issue and discussion forms, the pull request template and the links of the contributor kit, and that this table and the uncached commands below list every task the gate runs, and the tests of the release tooling under `scripts/release/`, among them a `set-version` test that runs `cargo` in a two-crate fixture and `index.test.mjs`, which holds the committed `release-notes/index.json` to its shape), `fragments` (`node scripts/release/check-fragments.mjs`: every change fragment under `changes/` follows the schema in [docs/release-management.md](release-management.md#writing-a-change-fragment)), `decisions` (`node scripts/decisions.mjs --check`: every decision file in `factory/decisions/` has a well-formed name, heading and fields, and every `Supersedes:` names a file that exists; `pnpm decisions` reads the same folder), `test-changes` (`node scripts/check-test-changes.mjs`: compares `HEAD` with `origin/main` and fails when the branch removes a test, skips one, removes an assertion or changes an expected value and none of its commits carries a `Test-change: <reason>` trailer; a checkout without `origin/main` compares nothing and says so, and the `changes` check runs it on every pull request; never cached), `texts` (`node scripts/texts.mjs --check`: every entry under `texts/`, the one place of every text a developer reads from the SDKs, the command line and the core, one file per text at `texts/<kind>/<name>.json`, has its kind (a notice, an error message or a part of one, among others), its text and when it shows, and a notice or an error its code, and each file is written one field per line, so `git diff` shows each change; the generated modules, `crates/nocentraldb/src/texts/` among them, equal the files, list the texts by a hash of their ids ([texts/README.md](../texts/README.md)) and hold no file longer than the limit `file-size` reads, the SDK's error texts split into four files by a hash of their ids for that (NOC-541); no notice or error text is written in the Rust SDK's code, outside the bridge, whose texts come with the TypeScript SDK's; and every text is used), `version` (`node scripts/release/check-version.mjs`: `Cargo.toml` `[workspace.package]` holds a plain `X.Y.Z`, every crate inherits it, dependencies between workspace crates are pinned to it in `[workspace.dependencies]`, and every workspace `package.json` and `Cargo.lock` carry it; see [Versioning](release-management.md#versioning)), `api-report` (`node scripts/release/api-report.mjs`: the TypeScript SDK's public declarations, regenerated with `tsc`, equal the committed `packages/sdk/etc/api-report.d.ts`), `wasm-runner` (`cargo install wasm-bindgen-cli --version =0.2.130 --locked --root target/tools`: installs the runner that `test-wasm` needs, at the `wasm-bindgen` version `Cargo.lock` holds; a cold run builds `wasm-bindgen-cli` once, which takes about a minute, and later runs reuse it from the cached `target/`), `site` (`pnpm --dir site install --frozen-lockfile`, `pnpm --dir site build`, then `node --test 'site/scripts/*.test.mjs'`: the site, its own pnpm project, installs from its lockfile and builds, and its built output holds the landing page and the docs, `/llms.txt` and `/llms-full.txt` naming `https://nocentraldb.io/`, a Markdown copy of every docs page and the `CNAME`; `.github/workflows/site-pages.yml` runs the same steps before it deploys the site to GitHub Pages); `rust-toolchain` runs before every Rust task and before `test` and `wasm-runner` |

Any formatting difference, clippy warning, Biome diagnostic (warnings included), type error or failing test fails the gate. Cargo commands that resolve dependencies run with `--locked`, so a `Cargo.toml` change without the matching `Cargo.lock` fails.

Running one gate on its own, uncached:

```sh
moon run ncdb-core:format-check --force
moon run ncdb-core:lint --force
moon run ncdb-core:test --force
moon run ncdb-core:build-wasm --force
moon run ncdb-core:test-wasm --force
moon run ncdb-core:lint-rich-text --force
moon run ncdb-core:test-rich-text --force
moon run ncdb-core:test-wasm-rich-text --force
moon run ncdb-plugin:format-check --force
moon run ncdb-plugin:lint --force
moon run ncdb-plugin:test --force
moon run ncdb-plugin:build-wasm --force
moon run ncdb-plugin:test-wasm --force
moon run ncdb-harness:format-check --force
moon run ncdb-harness:lint --force
moon run ncdb-harness:test --force
moon run ncdb-harness:build-wasm --force
moon run ncdb-harness:test-wasm --force
moon run ncdb-test-counter:format-check --force
moon run ncdb-test-counter:lint --force
moon run ncdb-test-counter:test --force
moon run ncdb:format-check --force
moon run ncdb:lint --force
moon run ncdb:test --force
moon run ncdb-wasm:format-check --force
moon run ncdb-wasm:lint --force
moon run ncdb-wasm:test --force
moon run ncdb-wasm:build-wasm --force
moon run ncdb-relay:format-check --force
moon run ncdb-relay:lint --force
moon run ncdb-relay:test --force
moon run ncdb-relay-server:format-check --force
moon run ncdb-relay-server:lint --force
moon run ncdb-relay-server:test --force
moon run ncdb-embedder-example:format-check --force
moon run ncdb-embedder-example:lint --force
moon run ncdb-embedder-example:test --force
moon run ncdb-relay-aws:format-check --force
moon run ncdb-relay-aws:lint --force
moon run ncdb-relay-aws:test --force
moon run ncdb-relay-lambda:format-check --force
moon run ncdb-relay-lambda:lint --force
moon run ncdb-relay-lambda:test --force
moon run ncdb-relay-lambda:test-sdk-sync --force
moon run nocentraldb:format-check --force
moon run nocentraldb:lint --force
moon run nocentraldb:test --force
moon run nocentraldb:build-wasm --force
moon run nocentraldb:test-wasm --force
moon run nocentraldb:lint-rich-text --force
moon run nocentraldb:test-rich-text --force
moon run nocentraldb-wasm:format-check --force
moon run nocentraldb-wasm:lint --force
moon run nocentraldb-wasm:test --force
moon run nocentraldb-wasm:build-wasm --force
moon run nocentraldb-wasm:test-wasm --force
moon run nocentraldb-wasm:build-wasm-rich-text --force
moon run nocentraldb-wasm:lint-rich-text --force
moon run sdk:format-check --force
moon run sdk:lint --force
moon run sdk:typecheck --force
moon run sdk:test --force
moon run infra-aws:format-check --force
moon run infra-aws:lint --force
moon run infra-aws:typecheck --force
moon run infra-aws:test --force
moon run repo:format-check --force
moon run repo:pins --force
moon run repo:file-size --force
moon run repo:test --force
moon run repo:fragments --force
moon run repo:decisions --force
moon run repo:test-changes --force
moon run repo:texts --force
moon run repo:version --force
moon run repo:api-report --force
moon run repo:site --force
moon run repo:wasm-runner --force
moon run repo:rust-toolchain --force
```

Re-running one test in isolation, e.g. to check a suspected flake:

```sh
cargo test --locked -p ncdb-core <test name>
pnpm --filter @nocentraldb/sdk exec vitest run -t "<test name>"
node --test --test-name-pattern "<test name>" scripts/check-pins.test.mjs
node --test --test-name-pattern "<test name>" scripts/ci-workflow.test.mjs
node --test --test-name-pattern "<test name>" scripts/changes-workflow.test.mjs
node --test --test-name-pattern "<test name>" scripts/release-workflow.test.mjs
node --test --test-name-pattern "<test name>" scripts/release-workflow-alpha.test.mjs
node --test --test-name-pattern "<test name>" scripts/release-workflow-publish.test.mjs
node --test --test-name-pattern "<test name>" scripts/contributor-kit.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/lib/fragments.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/check-fragments.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/lib/glob.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/lib/git.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/lib/semver.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/lib/manifests.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/check-version.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/set-version.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/next-version.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/render-notes.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/draft-notes.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/cut.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/promotion-check.test.mjs
node --test --test-name-pattern "<test name>" scripts/release/api-report.test.mjs
```

### The merge queue

A pull request reaches `main` through GitHub's merge queue (NOC-511). Once the merge rule in `factory/CONFIG.md` is met, pm adds the pull request to the queue instead of merging it. The queue builds a commit of its own, on a branch `gh-readonly-queue/main/pr-<number>-<sha>`: `main` as it stands, the pull requests ahead of it in the queue, and this pull request, squashed. The required checks `gates` and `changes` run on that commit (the `merge_group` event), and when both pass, the queue moves `main` to it, in queue order. A failing pull request leaves the queue, and the ones behind it are built again without it. Nothing is pushed to the pull request's branch, so the owner's approval stays: a pull request no longer needs a merge of `main`, a second CI run and a second approval because another one merged first.

Only `ci` and `changes` run in the queue. The release, deploy and pages workflows never do, and the alpha release follows a `ci` run on `main` only (`scripts/merge-queue-workflows.test.mjs`). The browser workflows run on pull requests that touch their paths; `merge_group` takes no path filter, and they are not required checks.

Agents cannot change branch rules. The owner sets the queue in the ruleset that protects `main` (Settings → Rules → Rulesets, "Require PR"):

- **Require merge queue** on, with merge method **Squash**.
- **Status checks that are required**: `gates` and `changes`, as today. The ruleset must not require a check that does not run on `merge_group`, or the queue waits for it.
- **Build concurrency** and the **batch size** (the minimum and maximum pull requests the queue merges at once, and how long it waits for them), as the owner decides them (NOC-515). One pull request at a time merges at most about two an hour at today's gate time of about 30 minutes; larger batches merge faster, but a failing batch is split up and built again.
- **Status check timeout**: above the `gates` job's 45 minutes, such as 60 minutes.

### Caching and what moon buys

moon gives the repository one task graph over Rust and TypeScript, runs independent tasks in parallel, and caches task results for the inner loop (`pnpm gates:dev`). Independent checks (`pnpm gates`, CI) do not trust moon's task cache, but they still reuse the self-validating caches of the compilers and package manager (Cargo's `target/`, the pnpm store), so compilation stays incremental there too.

For the inner loop, every task also takes the root files it depends on as inputs: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` for Rust tasks; `biome.json`, `tsconfig.base.json`, `package.json`, `pnpm-lock.yaml`, `.prototools` for TypeScript tasks. Every Rust task takes the sources of ALL crates as inputs: coarse, but a change in one crate re-runs the checks of every crate that may depend on it. Tracking the installed Rust toolchain beyond `rust-toolchain.toml` is best effort; a stale inner-loop result is caught by `pnpm gates`.

### Releases

How versions, change fragments, release notes and the alpha, beta and release stages work, and who does what at each stage, is in [docs/release-management.md](release-management.md). Its tooling is being built; the page marks every part that is not active yet. Alphas are active: every merge to `main` that adds a fragment other than `internal` becomes a tag `vX.Y.Z-alpha.N` and a GitHub prerelease once `ci` is green (`node scripts/release/next-version.mjs` shows what the next one would be); betas and releases are not yet active.

Versioning: everything the project publishes shares one version, held as a plain `X.Y.Z` (today `0.1.0`) in `Cargo.toml` `[workspace.package]` and every workspace `package.json`; `repo:version` checks it and `node scripts/release/set-version.mjs <version>` writes it (see [Versioning](release-management.md#versioning)).

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
| crates.io | `ncdb-plugin` | placeholder; the plugin crate in `crates/ncdb-plugin` takes it over with its first real release |
| crates.io | `ncdb-harness` | placeholder; the harness crate in `crates/ncdb-harness`, which RFC 0011 plans, takes it over with its first real release |

