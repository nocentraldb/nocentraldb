# Release management

How NoCentralDB is versioned, how a change reaches a user, and who does what at each stage. This page is for maintainers and for the agents working in this repository. Why the process looks like this (goals, anti-goals, the approaches that were weighed) is in the epic `mt08.q8rt` (`nxf show mt08.q8rt`), not here.

Most of the tooling described below does not exist yet. Every part that is not built yet says so: **not yet active, built by `<ticket>`** (`nxf show <ticket>`). Until it is built, nothing on this page should be read as a tool you can run. The ticket that builds a part completes its section here in the same pull request.

## Overview

NoCentralDB publishes a Rust core (`ncdb-core`), a Rust SDK (`nocentraldb`), the TypeScript SDK `@nocentraldb/sdk`, the CLI `ncdb` and a relay, all under one version. A change goes through three stages. **Alpha:** every merge to `main` that changes the product becomes an alpha automatically, so it can be tried at once. **Beta:** when a planned release's scope is met, the PM proposes a beta, the owner approves it, `release/X.Y` is cut from `main`, and the owner tags the beta on that branch. **Release:** when the promotion conditions hold, the PM proposes the release and the owner tags it on the head of `release/X.Y`.

Every promotion is a tag pushed by the owner, never a click in the GitHub UI and never a follow-up pull request. The GitHub release, the registry packages and the archived notes are consequences of the tag, produced by CI from a commit CI tested.

![Release flow: alpha on every merge to main, beta on the release branch after the owner's approval, release after the promotion conditions, and where each stage is published](release-flow.svg)

The diagram is plain SVG (no Mermaid); GitHub renders it here and it follows the reader's light or dark colour scheme.

## Versioning

**Status:** the rules below are the process. The manifest version and its checks (`check-version`, `set-version`, `repo:version`) are not yet active, built by `mt08.b3j4`. The computation of the next version (`next-version`) is not yet active, built by `mt08.x49q`.

- SemVer with prerelease suffixes. Tags look like `v0.3.1-alpha.4`, `v0.3.1-beta.2`, `v0.3.1`. The stage is part of the version, so the registries understand it. Any other tag shape is refused.
- One version for everything the project publishes (crates, npm packages, CLI, relay), always in lockstep. The unscoped npm `nocentraldb` stays a parked `0.0.0` placeholder outside the shared version.
- **Where the version lives:**
  - Committed manifests hold only the plain version `X.Y.Z`, never a stage suffix: `[workspace.package] version` in `Cargo.toml` (every crate inherits it with `version.workspace = true`) and the `version` of every `package.json` under `packages/*` and `apps/*`. Inter-crate dependencies are declared in `[workspace.dependencies]` with `path` and `version = "=<version>"` and used as `name.workspace = true`.
  - CI writes the full version (`X.Y.Z-alpha.N`, `X.Y.Z-beta.N` or `X.Y.Z`) into the manifests of its build checkout from the tag, right before building and publishing, and refreshes the workspace members' entries in `Cargo.lock` with `cargo update --workspace --offline`. Nothing about the stage is committed; that is what lets a release sit on exactly the beta's product code.
  - A gate task fails when the manifests disagree with each other. For beta and release tags the release workflow also fails when the tag's `X.Y.Z` differs from the manifests.
- **The first real release is `0.1.0`**, above the `0.0.0` placeholders on the registries.
- **The next version** is derived from the change fragments since the last release:
  - The last release is read from `release-notes/index.json`: the highest entry with stage `release`. It is never inferred from the existence of a `vX.Y.Z` tag (a refused release tag stays in place). Before the first release there is none.
  - Any `breaking` fragment bumps the minor version before 1.0 and the major version from 1.0 on; any `feature` bumps the minor version; otherwise the patch version is bumped.
  - The target version is the higher of that bump result and the manifest version on `main`. Before the first release every alpha is `0.1.0-alpha.N` whatever the fragment types, because the manifests say `0.1.0`. After a cut, `main`'s manifests already say `X.(Y+1).0`.
  - Alpha and beta numbers count up within the target version: `N` is one above the highest existing `v<target>-alpha.N` (or `-beta.N`) tag. Alpha numbers come from the existing alpha tags, because alphas are never written to the index; a gap after a failed run is harmless.
  - Example of a target change: after 1.2.0 ships, `main` says 1.3.0 and produces `1.3.0-alpha.N`; a `breaking` fragment arrives, the target becomes 2.0.0 and the next alpha is `2.0.0-alpha.1`.
  - The alpha job computes all this itself and commits nothing. The committed manifest version is raised only when a release branch is cut.

## Branches

- `main` always moves forward and produces alphas of the next version.
- `release/X.Y` stabilises one minor line and ships its patches. Betas and releases come from a release branch only. Older lines are not maintained in parallel (no long-term support before 1.0).
- **Cutting the branch:** when the owner approves a beta, `release/X.Y` is cut from `main` at the merge commit of the approved release-notes pull request (see [Beta](#beta)).
  - On the branch, one commit sets the manifests to `X.Y.Z`. For the first cut of a minor line the manifests on `main` already say `X.Y.0`, and no commit is needed.
  - On `main`, a pull request sets the manifests to `X.(Y+1).0`, so `main`'s alphas sort above the release line. It carries the `skip-changes` label, because it changes nothing a user gets.
  - Who pushes the branch: the cut tool creates `release/X.Y` locally and prints the push command; the owner, or the PM on the owner's word, pushes it. The cut tool is not yet active, built by `mt08.4fp9`.
- **Fixes during beta** land on `main` first, with their fragment, and are cherry-picked to the branch, fragment included. Nothing is on the branch that is not on `main`, except the release notes and the manifest version.
- **Which fragments belong to which release:** the fragments present at the cut, plus every fragment cherry-picked to the branch afterwards. After the release, the release workflow's commit on `main` removes every fragment that shipped in `X.Y.Z`, so the next release's notes never repeat it.
- **Patch releases** `X.Y.(Z+1)` follow on the same branch, through beta again: the owner approves, the branch's manifests are set to `X.Y.(Z+1)`, a new notes file `release-notes/X.Y.(Z+1).md` is drafted, and `vX.Y.(Z+1)-beta.1` is pushed on the branch head.
- Never commit `NEXUS_MEMORY.md` (or anything else that is not a cherry-picked fix, the release notes or the version) on a `release/*` branch.

## Writing a change fragment

**Checks:** the schema check is active: `node scripts/release/check-fragments.mjs` (the gate task `repo:fragments`, part of `pnpm gates`) validates every fragment in `changes/`, and `changes/README.md` is the writer's guide. The required-fragment check `changes` (workflow `.github/workflows/changes.yml`), its allowlist and the `skip-changes` label are active. Not yet active: the breaking-change detection, built by `mt08.nrt0`.

Every pull request that changes the product carries one change fragment.

- **Location:** `changes/<slug>.md`, YAML front matter and a Markdown body. The slug matches `^[a-z0-9][a-z0-9-]*$`. `changes/README.md` is the short writer's guide; the full rules are on this page.

Front matter:

| field | values | required |
|---|---|---|
| `type` | `breaking`, `feature`, `fix`, `deprecation`, `security`, `internal` | yes |
| `tickets` | nxf ids, e.g. `[mt08.1qaz]`; each matches `^[a-z0-9]+\.[a-z0-9]+$` | yes |
| `introduced` | for `fix`: `cycle` (the bug only existed since the last release) or the released version `X.Y.Z` that introduced it | yes for `fix`, forbidden otherwise |
| `components` | non-empty list from `core`, `sdk-rust`, `sdk-ts`, `cli`, `relay` | yes |

Required body sections (level-2 headings) by type:

- `breaking` and `feature`: **Problem** (what a user could not do or did badly), **Use** (how to use it, with a short example). `breaking` also **Migration** (what to change).
- `fix`: **Symptom** (what went wrong), **When** (in which situations it was visible), **Cause** (why it existed), **Fix** (how it was solved).
- `deprecation`: **Deprecated**, **Replacement**, **Removed in** (the earliest version that removes it).
- `security`: **Description** (short, public) and **Advisory** (the advisory id once published; `pending` until then).
- `internal`: one line, no sections. Never shown in curated notes.

The checks:

- **Schema check (gate task `repo:fragments`, `node scripts/release/check-fragments.mjs`):** every `changes/*.md` except `README.md` parses, has the required fields with allowed values and no other field, and has exactly the body sections its type requires (level-2 headings; a `## ` line inside a fenced code block is not a section). Runs locally in `pnpm gates` and in CI. A malformed fragment turns `pnpm gates` red; each violation is one line, `check-fragments: changes/<slug>.md: <rule>`. A repository without `changes/` passes.
- **Required-fragment check (workflow `.github/workflows/changes.yml`, check name `changes`, on pull requests to any base):** the pull request's changed paths are compared with the allowlist below. If any path is outside it and the pull request adds no fragment under `changes/`, the check fails and names each such path: `check-fragments: <path> ships to a user and the pull request adds no fragment under changes/ (add one, or label the PR skip-changes)`. The check needs only `contents: read`; labels come with the event payload. In detail:
  - The changes are diffed from the merge base of the pull request's base and head, so commits on the base branch after the fork never count. Deleted paths count; a moved file counts as its old and its new path.
  - Only a fragment the pull request *adds* (a new `changes/<slug>.md`) satisfies the rule. Editing or renaming an existing fragment does not, nor does a file under `changes/` that is not a fragment (`README.md`, a bad slug).
  - Every fragment the pull request adds or modifies is validated against the schema as it stands at the head commit, with or without the label.
  - The workflow runs `node scripts/release/check-fragments.mjs --base <base sha> --head <head sha> [--skip]`; the same command reproduces it locally (e.g. `--base origin/main --head HEAD`).
- **Breaking-change detection** (also in `changes`): `cargo-semver-checks` compares each workspace crate against the pull request's base commit, and the TypeScript SDK's API report is regenerated and diffed. A reported breaking Rust change, or a removed or changed line in the API report, fails `changes` unless the pull request adds a `breaking` fragment, naming the item.
- **The `skip-changes` label** on the pull request passes the required-fragment check (the workflow passes `--skip`). Adding or removing it re-runs the check (the workflow runs on `labeled` and `unlabeled`). It never waives a detected breaking change: that still needs a `breaking` fragment (`--require-breaking` ignores `--skip`). The label exists on the repository; it is meant for pull requests that change product paths but ship nothing to a user, such as a version raise or a lockfile refresh.

**Allowlist** (no fragment needed when every changed path matches): `docs/**`, `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `NEXUS_MEMORY.md`, `LICENSE-*`, `.github/**`, `scripts/**`, `moon.yml`, `**/moon.yml`, `.moon/**`, `biome.json`, `tsconfig.base.json`, `.prototools`, `.nvmrc`, `.gitignore`, `**/tests/**`, `**/test/**`, `**/*.test.*`, `changes/**`, `release-notes/**`. Everything else ships to a user and needs a fragment or the label; that includes `Cargo.toml`, `Cargo.lock`, every `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `rust-toolchain.toml` and every `src/`. Lockfile updates (Dependabot included, once it covers them) need a fragment or the label too.

**Lifecycle:** fragments are not deleted at alpha or beta. On `release/X.Y` the release's fragments are removed in the last notes pull request before the release tag. On `main` they are removed by the release workflow's commit after the release. Git history keeps them.

## Release notes

**Status:** the format below is the process. The index file, its test and the curated-notes rendering are not yet active, built by `mt08.ss3j`; the skeleton generator `draft-notes` by `mt08.4fp9`.

- **One Markdown file per release version**, `release-notes/<X.Y.Z>.md`. It is the same file from `beta.1` to the release, edited on `release/X.Y` before each beta, and archived on `main` by the release.
- **"Changes since beta N-1"** in a beta's GitHub notes is generated at publish time from the fragments added on the branch since the previous beta tag, and never committed.
- **`release-notes/index.json`** lists one entry per published beta and release, newest last (sorted ascending, tags unique):

  ```json
  { "releases": [ { "version": "0.1.0", "stage": "beta", "tag": "v0.1.0-beta.1", "date": "YYYY-MM-DD", "path": "release-notes/0.1.0.md" } ] }
  ```

  `stage` is `beta` or `release`. `tag` lets a consumer fetch the file as it was at that tag. `date` is a UTC calendar day. The release workflow updates the file and commits it to `main`; adding an identical entry again is a no-op. A test in `repo:test` holds the committed file to this shape.
- **Order of sections** in curated notes:
  1. a two- or three-sentence summary of the release;
  2. **Breaking changes** (each with problem, use, migration);
  3. **New features** (problem, then use);
  4. **Deprecations**;
  5. **Fixes** (symptom, when, cause, fix);
  6. **Security**;
  7. **Known issues** (bugs introduced in this cycle, known and not yet fixed, each with a workaround if there is one).
- **The filter:** a `fix` with `introduced: cycle` never appears in curated notes, because a user of the previous release never saw that bug. It still appears in alpha notes and in "Changes since beta N-1". `internal` never appears in curated notes.
- **Tone:** factual, written for the user of the product. It may say why a feature is worth having but makes no claims the product does not back. Shorter is better: one paragraph per entry, plus a code block where it helps. English only.
- **Where the notes appear:**
  - **GitHub:** the release body is the file's content, plus the generated "Changes since beta N-1" section for betas after the first. Alpha prereleases carry the alpha notes instead (see [Alpha](#alpha)).
  - **CLI** (contract for the `ncdb` crate, built when the CLI exists): `ncdb changes` prints the notes of the installed version; for an installed beta, the curated file as it was at that beta's tag (via `tag`). `ncdb changes <from>..` prints the curated notes of every release after `<from>` from `index.json` and the files on `main`, never alpha fragments. The CLI may print a one-line hint when a newer release exists. It renders the Markdown for the terminal itself.
  - **Landing page** (contract, built when the landing page exists): shows released versions only, rendered from `index.json` and the files on `main`, plus the roadmap from open milestones.

## Alpha

**Status:** not yet active, built by `mt08.x49q` (the `release` workflow's job `alpha`, `next-version`, `render-notes --alpha`). Until then no alpha is produced.

There is nothing to do by hand. Merge a product-changing pull request with its fragment to `main`; the alpha follows.

What the job does: the `release` workflow runs on `workflow_run` of the `ci` workflow (`types: [completed]`, `branches: [main]`) and proceeds only when that run's `conclusion` is `success`. It checks out the run's `head_sha` with full history and tags, in a concurrency group `alpha` that never cancels a run in progress, and then:

1. **Guard:** if the commit is no longer the head of `origin/main`, it stops with a neutral result and a log line.
2. Computes the next version (see [Versioning](#versioning)). It commits nothing.
3. Collects the fragments added since the previous alpha tag, the nearest alpha tag reachable from the commit (`git diff --diff-filter=A --name-only <previous alpha tag> HEAD -- changes/`; before the first alpha, every fragment present). If none of them has a `type` other than `internal`, it stops with a neutral result.
4. Tags `v<next>-alpha.<n>` on the commit and pushes the tag.
5. Publishes a GitHub prerelease whose body is the alpha notes: those fragments rendered as Markdown, each under a heading made from its slug, plus a link to the previous alpha's release. CLI and relay binaries are attached once they exist; npm gets the alpha under the dist-tag `alpha` once the repository is public and the SDK is publishable (see [Not yet active](#not-yet-active)).

Alphas never go to crates.io: crates.io versions are permanent, and one per merge would bury the real versions. Rust users who want an alpha depend on the git tag.

When no alpha appears, and why that is fine:

- **All-internal merges:** a merge whose fragments are all `internal`, or that needed no fragment, produces no alpha.
- **The commit is no longer the head of `main`:** two merges in quick succession must not produce an alpha with a higher number on older code, so the older one is skipped. The next alpha covers it, because alpha notes are computed since the previous alpha tag, not per merge. Nothing is lost.
- **The workflow's own pushes to `main`** (the release workflow's commit after a beta or release) can delay an alpha by one run.

A tag pushed with the workflow's own `GITHUB_TOKEN` does not start another workflow run; the publish job also ignores `-alpha.` tags explicitly.

**Dry run** (once the workflow exists): `gh workflow run release.yml -f tag=` — a `workflow_dispatch` with an empty `tag` input computes and prints everything and tags and publishes nothing. A dispatch is always a dry run.

## Beta

**Status:** the steps are the process. The tools are not yet active: `draft-notes` and `cut` are built by `mt08.4fp9`, the publish job by `mt08.a4xv` (with its scripts from `mt08.ss3j`). Registry publishing waits for the repository to go public (`mt08.1213`).

1. **Propose (PM).** When the release epic is no longer blocked, the PM checks the epic's description against what was delivered. If the scope is met, it proposes a beta to the owner and attaches a draft of the release notes; if not, it says what is missing.
2. **Draft (PM or agent).** Draft `release-notes/<X.Y.Z>.md` (see [Release notes](#release-notes)) from two sources: every fragment since the previous release, rendered into the skeleton by `draft-notes` with the filter applied and the sections in order, and every board item created or closed since the release epic was created (read with `nxf`; Known issues in particular).
3. **Pull request to `main`.** The draft goes into a pull request against `main`. The owner edits and approves it there; merging it is the approval.
4. **Cut (`cut`).** On the merge, `release/X.Y` is cut from the merge commit (see [Branches](#branches)). The cut tool sets the manifests on the branch and prints the push command and the command for the pull request that raises `main` to `X.(Y+1).0` (label `skip-changes`).
5. **Push the branch.** The owner, or the PM on the owner's word, pushes `release/X.Y`.
6. **Tag (owner).** The owner pushes the beta tag on the branch head:

   ```sh
   git tag vX.Y.Z-beta.1 && git push origin refs/tags/vX.Y.Z-beta.1
   ```

What the publish job does on the tag: the `release` workflow runs on `push` of tags `v*`, excluding `-alpha.` tags. It runs the full gate `pnpm gates` on the tagged commit itself, checks the tag against the manifests, writes the full version into the build checkout, builds, and publishes a GitHub prerelease whose body is the curated file, plus, for betas after the first, the generated section "Changes since beta N-1" (the fragments added on the branch between the two beta tags: `git diff --diff-filter=A --name-only <previous beta tag> <this tag> -- changes/`). Once the repository is public and the packages are publishable, it also publishes to npm under the dist-tag `beta` and to crates.io as a prerelease version. It then appends the beta to `release-notes/index.json` on `main`.

A tag whose `X.Y.Z` differs from the manifests turns `publish` red before anything is built or published. The tag is left in place for the owner to delete or move.

**Fixes during beta:**

- Land the fix on `main` first, with its fragment (there it produces alphas of the next version).
- Cherry-pick it to `release/X.Y`, fragment included.
- The fragment carries `introduced: cycle` if the bug only existed since the last release, or `introduced: X.Y.Z`, the released version that introduced it.

**Later betas:** the curated file on the branch is updated in a pull request to the branch before each beta tag; then the owner pushes `vX.Y.Z-beta.2`, `vX.Y.Z-beta.3`, … on the branch head.

**Checks on the release branch:** pull requests to `release/X.Y` run `ci` and `changes` exactly like pull requests to `main` (`ci.yml:12` triggers on every `pull_request`). Only pushes to a release branch do not run `ci`; that is why the publish job runs the full gate on the tagged commit itself.

## Release

**Status:** the checklist is the process. `promotion-check` is not yet active, built by `mt08.4fp9`; the publish job and its commit back to `main` by `mt08.a4xv`; the upgrade test waits for the relay (`mt08.nptg`, behind `mt08.7wjb`).

The PM checks the promotion conditions in its sessions and proposes the release to the owner when all hold. The owner decides.

| # | Condition | Who checks |
|---|---|---|
| 1 | **Soak:** the latest beta has been out for at least 7 days with no newer beta (from the GitHub release dates via `gh`). | `promotion-check` |
| 2 | **No open regressions:** no open bug of P0 or P1 whose `introduced` is this cycle. Lower ones are listed under Known issues. | the PM, on the board |
| 3 | **Upgrade tested:** an automated test upgrades from the previous release to the candidate: stored data, `model.lock.json`, an old client against the new relay, a new client against the old relay, convergence throughout. | the upgrade test |
| 4 | **Same product code:** the release is tagged on the head of `release/X.Y`; since the last beta tag only `release-notes/` and `changes/` have changed. | `promotion-check` (and the publish job refuses the tag otherwise) |
| 5 | **Security:** `cargo audit` and `pnpm audit` are clean and no security advisory is open. A missing `cargo-audit` is a failed condition, never a pass. | `promotion-check` |
| 6 | **Notes approved:** the curated notes are final and approved by the owner; every breaking change has a Migration section. | the PM, in the notes pull request |
| 7 | **Real use:** at least one of our own projects runs the beta with no new bug report. | the PM, on the board |

Before 1.0, conditions 1, 2, 4 and 6 apply from the first release, and so does 5. Conditions 3 and 7 apply as soon as there is a relay and a real user.

For condition 2, open bugs have no fragment yet, so the PM derives `introduced` from the bug ticket's description: when it was first seen, and which version or change it came with. If the description does not say, the PM asks rather than guesses. The bug's fragment records the value when the fix lands.

Steps:

1. **Final notes pull request** to `release/X.Y`: the curated notes as final, and the deletion of the release's fragments from `changes/`. A release is refused while `changes/` on the branch is not empty.
2. **Tag (owner):** on approval, the owner pushes `vX.Y.Z` on the head of `release/X.Y`:

   ```sh
   git tag vX.Y.Z && git push origin refs/tags/vX.Y.Z
   ```

3. **What the publish job does:**
   1. runs the gate, checks tag = manifests, checks the diff rule of condition 4 (`git diff --name-only <last beta tag> <release tag>` may only show `release-notes/` and `changes/`), and requires `release-notes/<X.Y.Z>.md` to exist at the tag. Any failure turns `publish` red before anything is built or published, and the tag is left in place for the owner to delete or move;
   2. writes the version into the checkout, builds (the version is embedded, so the bytes differ from the beta's), and publishes: crates.io, npm under `latest`, and a GitHub release marked latest whose body is the file's content, without a manual step (the registries once the repository is public and the packages are publishable);
   3. commits back to `main`, in one commit pushed by the workflow: the updated `release-notes/index.json`, the final `release-notes/<X.Y.Z>.md` as it stands at the tag (so `main` is the archive the landing page renders from), and the removal of every fragment that shipped. The shipped fragments are exactly the files deleted under `changes/` on the branch between the merge base with `main` and the release tag (`git log --diff-filter=D --name-only --format= <merge-base>..<tag> -- changes/`); those still present on `main` are deleted;
   4. closes the GitHub milestone with `gh`; the landing page picks the change up from `main` once it exists.

   A push to `main` made with the workflow's token does not start the `ci` workflow. That commit touches only `release-notes/` and `changes/`; the fragment schema check and the index test cover those paths on the next pull request.
4. **When something fails:** a partial publish (the GitHub release exists, a registry step failed) leaves the job red and retracts nothing; re-run it, every publish step first checks whether its tag, release or package version already exists and skips it. If the commit to `main` fails to push because `main` moved, the job retries once after fetching; if it still fails, the job is red and the commit has to be made by hand (the steps are added here by `mt08.a4xv`).
5. **After the release:** the PM updates the `active-release` memory (see [Planning a release](#planning-a-release)).

**Patch releases** `X.Y.(Z+1)` go through the same branch and through beta again: the owner approves, the branch's manifests are set to `X.Y.(Z+1)`, `release-notes/X.Y.(Z+1).md` is drafted, and `vX.Y.(Z+1)-beta.1` is pushed on the branch head.

**Withdrawal:** a broken published version is yanked on crates.io and deprecated on npm, and a fixed version follows at once. Nothing is deleted.

## Planning a release

- **The release epic.** Each planned release has one epic on the board, e.g. "Release 0.3.0". Its description states what must be true for the release. Epics and tickets the release needs are attached as **dependencies**, never as children:

  ```sh
  nxf dep add <release-epic> <item>
  ```

  A dependency blocks; a parent only groups, and a deferred or closed parent would hide its children.
- **The milestone mirror.** The release epic is mirrored publicly as a GitHub milestone (title = the version, description = the public part of the epic) and as an entry on the landing page's roadmap once it exists. Until nexus-flow `6j6v.qv39` (GitHub Issues and milestone integration) exists, this is a manual step when the release is planned and whenever its scope changes. Which items appear publicly is decided per item; the default is not public. Milestones work on the private repository too; their visibility follows the repository's.

  ```sh
  # when the release is planned
  gh api repos/{owner}/{repo}/milestones -f title=<X.Y.Z> -f description=<text>
  # when its scope changes (<n> is the milestone's number)
  gh api -X PATCH repos/{owner}/{repo}/milestones/<n> -f description=<text>
  # at the release (the publish job does this once it exists)
  gh api -X PATCH repos/{owner}/{repo}/milestones/<n> -f state=closed
  ```

- **The `active-release` memory.** The release in progress is recorded as a project memory with the key `active-release`. Its body names the version, the release epic's id, the stage (`planned`; `beta` with the branch and the latest beta tag; or `released` with the date), and that the process is on this page. Its introduction line names the version and the epic id.

  The PM writes it, because the PM plans releases and is the one that reads the memory, at three moments:

  1. when a release is planned, right after the release epic is created (stage `planned`);
  2. when `beta.1` is tagged (stage `beta`, the branch, the tag), and after each further beta tag;
  3. when the release ships: the memory is rewritten for the next planned release if one exists, otherwise removed with `nxm forget active-release`.

  The command:

  ```sh
  nxm remember "<version>, epic <id>, stage <planned|beta: release/X.Y, vX.Y.Z-beta.N|released YYYY-MM-DD>; process: docs/release-management.md" --introduction "Active release <version> (epic <id>): stage <stage>" --key active-release
  ```

  After every write (and after `nxm forget active-release`), commit the regenerated `NEXUS_MEMORY.md` at once, staging only that path:

  ```sh
  git add NEXUS_MEMORY.md && git commit -m "Record the active release <version>" -- NEXUS_MEMORY.md
  ```

  Make that commit on `main` or on the current feature branch, **never on a `release/*` branch**: a commit there would break the same-product-code rule of promotion condition 4. The PM makes exactly this commit and no other change to the working tree.

  No `active-release` memory exists while no release is planned.

## The PM's duties

The PM persona receives `nxm prime` and `nxf prime` at session start, so it sees the active release and whether its epic is still blocked. These checks run in agent sessions, never in GitHub Actions: the board (`.nxs/`) is local and not synced, so CI cannot read it. The PM reads GitHub with `gh` for the facts that live there: CI status, release dates, tags. Nothing is pushed to the owner, because `nxc` cannot reach a person; once nexus-flow `6j6v.ek9g` (a way for `nxc` to notify the owner) exists, the PM can tell the owner about a ready beta or release without waiting for the next session.

- **Surface the active release** at session start, from the `active-release` memory and the board.
- **Check the scope** when the release epic is no longer blocked: raise it with the owner in the next session, compare the epic's description with what was delivered, and either propose a beta with a draft of the release notes or say what is missing.
- **Derive `introduced`** for open bugs from the bug ticket's description (when it was first seen, which version or change it came with), and ask when the description does not say; never guess.
- **Run `promotion-check`** for conditions 1, 4 and 5 (not yet active, built by `mt08.4fp9`), and check conditions 2, 6 and 7 on the board and in the notes pull request.
- **Propose** the beta and the release to the owner; the owner decides and pushes the tag.
- **Write the `active-release` memory** at the three moments above and commit `NEXUS_MEMORY.md`, never on a `release/*` branch.
- **Mirror the release epic** as a milestone (see [Planning a release](#planning-a-release)).

## Supply chain and policies

1. **Provenance:** npm publishes with `--provenance`; binaries get GitHub artifact attestations (`actions/attest-build-provenance`). Both need a public repository.
2. **Breaking-change detection:** on every pull request, `cargo-semver-checks` compares each workspace crate against the base commit, and the TypeScript SDK's API report (committed under `packages/sdk/etc/`) is regenerated and diffed. A reported breaking change fails `changes` unless the pull request adds a `breaking` fragment. This works on a private repository; not yet active, built by `mt08.nrt0`.
3. **No long-lived tokens:** npm and crates.io are published to through Trusted Publishing (OIDC from GitHub Actions, `id-token: write` on the publish job only). The owner's passkey-protected accounts stay out of automation.
4. **Withdrawal:** a broken published version is yanked on crates.io and deprecated on npm, and a fixed version follows at once. Nothing is deleted.
5. **Security advisories:** through GitHub Security Advisories, with a Security section in the notes.
6. **Deprecation policy:** anything to be removed is marked deprecated, and announced in the notes, at least one release before it is removed.

**Published packages:** crates.io `ncdb-core` (core), `nocentraldb` (Rust SDK) and `ncdb` (the CLI, so `cargo install ncdb` works); npm `@nocentraldb/sdk`; GitHub releases carry the CLI and relay binaries. All share one version. A package that is not publishable (`private: true`, `publish = false`, or no package at all) is skipped by the publish steps without failing them.

**Going public.** The repository is private on the free GitHub plan today and goes public before the first real release, 0.1.0. On a private repository, GitHub releases, milestones and binaries are visible only to collaborators, npm provenance does not work, and artifact attestations need Enterprise Cloud. Until then alphas (and betas, if any) are published to GitHub only, visible to collaborators, and nothing is published to the registries. With going public these switch on (`mt08.1213`, `mt08.k1jh`, `mt08.may6`):

- Trusted Publishing for npm and crates.io with `id-token: write`, `--provenance`, artifact attestations;
- `@nocentraldb/sdk` becomes publishable (build output to `dist/`, `exports`, no `private: true`) and `ncdb-core` becomes publishable (no `publish = false`);
- branch protection with the `gates` and `changes` checks required, a tag ruleset for `v*`, and a bypass that lets the release workflow push its commit to `main`;
- the public milestone mirror.

**The owner's first publish.** Trusted Publishing can only be configured for a package that exists, so the owner publishes each package once, by hand, before it is configured. Every later publish goes through CI.

## Tooling reference

| Unit | Does | Status |
|---|---|---|
| `scripts/release/lib/` (`fragments.mjs`, `git.mjs`, `glob.mjs`, `test-repo.mjs`, `semver.mjs`, `manifests.mjs`) | shared parsing: fragments, git, allowlist globs, SemVer and tags, manifests; isolated git repositories for tests | `fragments.mjs`, `git.mjs`, `glob.mjs`, `test-repo.mjs` active; `semver.mjs`, `manifests.mjs` not yet active (`mt08.b3j4`) |
| `scripts/release/check-fragments.mjs [--dir <changes>]` | validates every `changes/*.md`; with `--base <rev> --head <rev> [--skip]` also applies the allowlist and the required-fragment rule; `--require-breaking` ignores `--skip` | schema mode and `--base/--head/--skip` active; `--require-breaking` exists, but nothing passes it until the breaking-change detection (`mt08.nrt0`) |
| `scripts/release/check-version.mjs [--tag <v…>]` | manifests agree on `X.Y.Z`; with `--tag`, tag = manifests for beta and release | not yet active (`mt08.b3j4`) |
| `scripts/release/set-version.mjs <version>` | writes the version into `Cargo.toml` `[workspace.package]` and every workspace `package.json`, then `cargo update --workspace --offline` | not yet active (`mt08.b3j4`) |
| `scripts/release/next-version.mjs [--json]` | the next alpha tag from tags, fragments and `release-notes/index.json` | not yet active (`mt08.x49q`) |
| `scripts/release/render-notes.mjs` | the GitHub release body: `--alpha`, and for beta and release | not yet active (`mt08.x49q`, `mt08.ss3j`) |
| `scripts/release/verify-tag.mjs` | the refusal rules for beta and release tags | not yet active (`mt08.ss3j`) |
| `scripts/release/index.mjs add --version --stage --tag --date` | appends to `release-notes/index.json` | not yet active (`mt08.ss3j`) |
| `scripts/release/shipped-fragments.mjs <release tag>` | lists the fragments that shipped | not yet active (`mt08.ss3j`) |
| `scripts/release/publishable.mjs` | which packages the publish steps may publish | not yet active (`mt08.ss3j`) |
| `scripts/release/commit-back.mjs` | the release workflow's commit to `main` | not yet active (`mt08.a4xv`) |
| `scripts/release/draft-notes.mjs <X.Y.Z>` | the curated-notes skeleton, filter applied, sections in order | not yet active (`mt08.4fp9`) |
| `scripts/release/cut.mjs <X.Y.Z>` | creates `release/X.Y`, sets the manifests, prints the push, raise-PR and beta-tag commands | not yet active (`mt08.4fp9`) |
| `scripts/release/promotion-check.mjs <X.Y.Z>` | reports promotion conditions 1, 4 and 5 with evidence | not yet active (`mt08.4fp9`) |
| `scripts/release/api-report.mjs`, `packages/sdk/etc/` | the TypeScript SDK's API report | not yet active (`mt08.nrt0`) |
| moon task `repo:fragments` | `node scripts/release/check-fragments.mjs` in `pnpm gates` | active |
| moon task `repo:version` | `node scripts/release/check-version.mjs` in `pnpm gates` | not yet active (`mt08.b3j4`) |
| moon task `repo:api-report` | the API report in `pnpm gates` | not yet active (`mt08.nrt0`) |
| moon task `repo:test`, widened | also runs the tests under `scripts/release/` (`node --test 'scripts/*.test.mjs' 'scripts/release/**/*.test.mjs'`) | active |
| workflow `changes.yml` (check `changes`) | required fragment, `cargo-semver-checks`, API report diff | required fragment active; `cargo-semver-checks` and the API report diff not yet active (`mt08.nrt0`) |
| workflow `release.yml`, job `alpha` | alpha on a green `ci` run on `main`; dispatch dry run | not yet active (`mt08.x49q`) |
| workflow `release.yml`, jobs `verify`, `build`, `publish`, `commit-back` | beta and release on `v*` tags that are not alphas; dispatch dry run | not yet active (`mt08.a4xv`) |
| `scripts/changes-workflow.test.mjs`, `scripts/release-workflow.test.mjs`, `scripts/lib/workflow-checks.mjs` | the workflows' contracts, with mutants | `changes-workflow.test.mjs` and `workflow-checks.mjs` active; `release-workflow.test.mjs` not yet active (`mt08.x49q`, `mt08.a4xv`) |
| label `skip-changes` | waives the required fragment, never a detected breaking change | active |
| `changes/README.md` | the fragments' writer guide | active |
| `release-notes/index.json` and its test | the notes archive index | not yet active (`mt08.ss3j`) |
| memory `active-release` | the release in progress, for the PM | written by the PM when a release is planned |

## Not yet active

What waits on something outside this repository's tooling. Each is a `WAIT:` chore on the board (`nxf show <id>`); the tickets that need it depend on it.

- `mt08.1ny6` — WAIT: repository goes public. Then: Trusted Publishing, provenance, attestations, publishable SDK and core crate (`mt08.1213`); rulesets for `main` and tags, and the release workflow's bypass (`mt08.k1jh`, after `mt08.may6`).
- `mt08.tjwz` — WAIT: the ncdb CLI exists. Then: `ncdb changes`, CLI binaries on every stage, `cargo install ncdb` (`mt08.h4rh`).
- `mt08.7wjb` — WAIT: the relay exists. Then: relay binaries on releases and the automated upgrade test of promotion condition 3 (`mt08.nptg`).
- `mt08.mn1j` — WAIT: the landing page exists. Then: the roadmap from milestones and the release notes from `index.json` and `main` (`mt08.zxg9`).

The Rust SDK needs no WAIT: the publish job publishes every publishable workspace crate, so `nocentraldb` ships when it exists and is publishable.
