# Release management

This page describes how NoCentralDB is versioned, how a change reaches a user, and who does what at each stage. It is for maintainers and for the agents that work in this repository. The epic `mt08.q8rt` (`nxf show mt08.q8rt`) explains why the process looks like this: its goals, anti-goals and the approaches we weighed.

Most of the tooling on this page does not exist yet. Each part that is not built says so, as **not yet active, built by `<ticket>`** (`nxf show <ticket>`). Until a part is built, do not treat its description as a tool you can run. The ticket that builds a part completes its section on this page in the same pull request.

## Overview

NoCentralDB publishes a Rust core (`ncdb-core`), a Rust SDK (`nocentraldb`), the TypeScript SDK `@nocentraldb/sdk`, the CLI `ncdb` and a relay, all under one version. A change goes through three stages.

- **Alpha.** Every merge to `main` that changes the product becomes an alpha automatically, so people can try it at once.
- **Beta.** When the scope of a planned release is met, the PM proposes a beta and the owner approves it. Then `release/X.Y` is cut from `main`, and the owner tags the beta on that branch.
- **Release.** When the promotion conditions hold, the PM proposes the release, and the owner tags it on the head of `release/X.Y`.

The owner promotes a version by pushing a tag, never by clicking in the GitHub UI and never through a follow-up pull request. From the tag, CI produces the GitHub release, the registry packages and the archived notes, built from a commit that CI tested.

![Release flow: alpha on every merge to main, beta on the release branch after the owner's approval, release after the promotion conditions, and where each stage is published](release-flow.svg)

The diagram is plain SVG, without Mermaid. GitHub renders it on this page, and it follows the reader's light or dark colour scheme.

## Versioning

**Status.** The rules below are the process. The manifest version and its checks are active. The manifests say `0.1.0`, `check-version` holds them to that version (the gate task `repo:version`, part of `pnpm gates`), and `set-version` writes a full version into a checkout. The computation of the next alpha is active as well: the alpha job runs `node scripts/release/next-version.mjs` (see [Alpha](#alpha)).

- Versions follow SemVer with prerelease suffixes. Tags look like `v0.3.1-alpha.4`, `v0.3.1-beta.2` and `v0.3.1`. The stage is part of the version, so the registries understand it. The tooling refuses any other tag shape.
- Everything the project publishes (crates, npm packages, CLI, relay) shares one version and always moves in lockstep. The unscoped npm package `nocentraldb` stays a parked `0.0.0` placeholder outside the shared version.
- **Where the version lives.**
  - Committed manifests hold only the plain version `X.Y.Z`, never a stage suffix. That is `[workspace.package] version` in `Cargo.toml`, which every crate inherits with `version.workspace = true`, and the `version` of every `package.json` under `packages/*` and `apps/*`. The workspace declares inter-crate dependencies in `[workspace.dependencies]` with `path` and `version = "=<version>"`, and each crate uses them as `name.workspace = true`.
  - CI writes the full version (`X.Y.Z-alpha.N`, `X.Y.Z-beta.N` or `X.Y.Z`) from the tag into the manifests of its build checkout, right before it builds and publishes. It then refreshes the workspace members' entries in `Cargo.lock` with `cargo update --workspace --offline`. Nothing about the stage is committed. That is why a release can use exactly the product code of its last beta.
  - A gate task fails when the manifests disagree with each other. For beta and release tags, the release workflow also fails when the tag's `X.Y.Z` differs from the manifests.
- **The checks.** `node scripts/release/check-version.mjs` (the gate task `repo:version`) fails and names the file when one of these holds:
  1. `Cargo.toml` has no `[workspace.package] version`, or the version is not a plain `X.Y.Z`. A `-` or `+` suffix fails, because committed manifests never carry a stage suffix.
  2. A `crates/*/Cargo.toml` declares its own `version`, or does not say `version.workspace = true`.
  3. A `crates/*/Cargo.toml` declares a `path =` dependency itself, in `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` or their `[target.….…]` forms.
  4. A `[workspace.dependencies]` entry with a `path` is not pinned with `version = "=<workspace version>"`.
  5. A `package.json` under `packages/*` or `apps/*` has a different `version`, or none.
  6. `Cargo.lock` does not list a workspace crate at the workspace version. Run `cargo update --workspace --offline` to fix it.
  7. With `--tag <tag>`, the tag is not `vX.Y.Z` or `vX.Y.Z-beta.N`, or its `X.Y.Z` is not the workspace version. The check refuses alpha tags, because alphas are never checked against the manifests.

  The check's reader of `Cargo.toml` and the crate manifests is narrow and fails closed. Inside the tables it reads, it understands only single-line keys of the shapes above. A multi-line value, a trailing comment on a line it parses, a dotted dependency key other than `name.workspace = true`, or a dependency sub-table such as `[workspace.dependencies.x]` fails with "unsupported manifest syntax at line N".
- **Dependencies between workspace crates.** Declare each one once, in `[workspace.dependencies]` of `Cargo.toml`, as `name = { path = "crates/name", version = "=X.Y.Z" }`, and use it in a crate as `name.workspace = true`. The exact `=` pin lets a prerelease resolve. Cargo matches the requirement `0.1.0`, which means `^0.1.0`, against no prerelease, so without the pin `0.1.0-beta.1` of a sibling crate would not resolve.
- **Writing a version.** `node scripts/release/set-version.mjs <version> [--no-lock]` does four things. It writes a full version (`X.Y.Z`, `X.Y.Z-alpha.N` or `X.Y.Z-beta.N`) into `[workspace.package]`. It pins every `[workspace.dependencies]` path entry to `=<version>` and inserts the pin where it is missing. It sets `version` in every `packages/*/package.json` and `apps/*/package.json`. Then it runs `cargo update --workspace --offline`, unless you pass `--no-lock`. It validates the version and reads every manifest before it writes any. The alpha and publish jobs run it in their build checkout. The cut runs it on the release branch, with a plain version.
- **The lock refresh offline.** We verified `cargo update --workspace --offline` on a two-crate fixture in the test of `set-version`, where it moves both workspace members to a prerelease version. The workspace has no third-party crates yet. The ticket that adds the first one must verify again that the command still succeeds offline with that crate in the lock. If it fails offline, `set-version` falls back to rewriting the `version` lines of the workspace members' `[[package]]` blocks in `Cargo.lock` directly, with the lock reader `lockVersions` of `scripts/release/lib/manifests.mjs`.
- **The first real release is `0.1.0`**, above the `0.0.0` placeholders on the registries.
- **The next version** follows from the change fragments since the last release.
  - The last release is the highest entry with stage `release` in `release-notes/index.json`. The tooling never infers it from the existence of a `vX.Y.Z` tag, because a refused release tag stays in place. Before the first release there is none.
  - A `breaking` fragment bumps the minor version before 1.0, and the major version from 1.0 on. A `feature` fragment bumps the minor version. Otherwise the patch version goes up.
  - The target version is the higher of that bump result and the manifest version on `main`. Before the first release every alpha is `0.1.0-alpha.N`, whatever the fragment types, because the manifests say `0.1.0`. After a cut, the manifests on `main` already say `X.(Y+1).0`.
  - Alpha and beta numbers count up within the target version. `N` is one above the highest existing `v<target>-alpha.N` (or `-beta.N`) tag. Alpha numbers come from the existing alpha tags, because alphas never go into the index. A gap after a failed run does no harm.
  - An example of a target change: after 1.2.0 ships, `main` says 1.3.0 and produces `1.3.0-alpha.N`. When a `breaking` fragment arrives, the target becomes 2.0.0, and the next alpha is `2.0.0-alpha.1`.
  - The alpha job computes all of this itself and commits nothing. The committed manifest version goes up only when a release branch is cut.

## Branches

- `main` always moves forward and produces alphas of the next version.
- `release/X.Y` stabilises one minor line and ships its patches. Betas and releases come only from a release branch. We do not maintain older lines in parallel, and there is no long-term support before 1.0.
- **Cutting the branch.** When the owner approves a beta, `release/X.Y` is cut from `main` at the merge commit of the approved release-notes pull request (see [Beta](#beta)).
  - On the branch, one commit sets the manifests to `X.Y.Z`. For the first cut of a minor line, the manifests on `main` already say `X.Y.0`, and no commit is needed.
  - On `main`, a pull request sets the manifests to `X.(Y+1).0`, so the alphas on `main` sort above the release line. It carries the `skip-changes` label, because it changes nothing a user gets.
  - The cut tool creates `release/X.Y` locally and prints the push command. The owner pushes the branch, or the PM does on the owner's word. The cut tool is not yet active, built by `mt08.4fp9`.
- **Fixes during beta** land on `main` first, with their fragment. Then they are cherry-picked to the branch, fragment included. The branch holds nothing that is not on `main`, except the release notes and the manifest version.
- **Which fragments belong to which release.** A release ships the fragments present at the cut, plus every fragment cherry-picked to the branch afterwards. After the release, the release workflow's commit on `main` removes every fragment that shipped in `X.Y.Z`, so the next release's notes never repeat it.
- **Patch releases** `X.Y.(Z+1)` follow on the same branch and go through beta again. The owner approves, the branch's manifests are set to `X.Y.(Z+1)`, a new notes file `release-notes/X.Y.(Z+1).md` is drafted, and `vX.Y.(Z+1)-beta.1` is pushed on the branch head.
- Never commit `NEXUS_MEMORY.md`, or anything else that is not a cherry-picked fix, the release notes or the version, on a `release/*` branch.

## Writing a change fragment

**Checks.** The schema check is active. `node scripts/release/check-fragments.mjs` (the gate task `repo:fragments`, part of `pnpm gates`) validates every fragment in `changes/`, and `changes/README.md` is the writer's guide. The required-fragment check `changes` (workflow `.github/workflows/changes.yml`), its allowlist and the `skip-changes` label are active too. The breaking-change detection is not yet active, built by `mt08.nrt0`.

Every pull request that changes the product carries one change fragment.

- **Location.** A fragment is a file `changes/<slug>.md` with YAML front matter and a Markdown body. The slug matches `^[a-z0-9][a-z0-9-]*$`. `changes/README.md` is the short writer's guide, and this page holds the full rules.

Front matter:

| field | values | required |
|---|---|---|
| `type` | `breaking`, `feature`, `fix`, `deprecation`, `security`, `internal` | yes |
| `tickets` | nxf ids, e.g. `[mt08.1qaz]`; each matches `^[a-z0-9]+\.[a-z0-9]+$` | yes |
| `introduced` | for `fix`: `cycle` (the bug only existed since the last release) or the released version `X.Y.Z` that introduced it | yes for `fix`, forbidden otherwise |
| `components` | non-empty list from `core`, `sdk-rust`, `sdk-ts`, `cli`, `relay` | yes |

Each type requires these body sections, as level-2 headings:

- `breaking` and `feature`: **Problem** (what a user could not do, or did badly) and **Use** (how to use it, with a short example). `breaking` also needs **Migration** (what to change).
- `fix`: **Symptom** (what went wrong), **When** (in which situations it was visible), **Cause** (why it existed) and **Fix** (how we solved it).
- `deprecation`: **Deprecated**, **Replacement**, and **Removed in** (the earliest version that removes it).
- `security`: **Description** (short and public) and **Advisory** (the advisory id once it is published, `pending` until then).
- `internal`: one line and no sections. Curated notes never show it.

The checks:

- **Schema check.** The gate task `repo:fragments` runs `node scripts/release/check-fragments.mjs`. It checks every `changes/*.md` except `README.md`. Each fragment must parse, must have the required fields with allowed values and no other field, and must have exactly the body sections its type requires, as level-2 headings. A `## ` line inside a fenced code block is not a section. The check runs locally in `pnpm gates` and in CI. A malformed fragment turns `pnpm gates` red, and each violation is one line, `check-fragments: changes/<slug>.md: <rule>`. A repository without `changes/` passes.
- **Required-fragment check.** The workflow `.github/workflows/changes.yml` runs the check named `changes` on pull requests to any base. It compares the pull request's changed paths with the allowlist below. If any path is outside the allowlist and the pull request adds no fragment under `changes/`, the check fails and names each such path: `check-fragments: <path> ships to a user and the pull request adds no fragment under changes/ (add one, or label the PR skip-changes)`. The check needs only `contents: read`, because the labels come with the event payload. In detail:
  - The check diffs from the merge base of the pull request's base and head, so commits on the base branch after the fork never count. Deleted paths count. A moved file counts as both its old and its new path.
  - Only a fragment that the pull request *adds*, as a new `changes/<slug>.md`, satisfies the rule. Editing or renaming an existing fragment does not, and neither does a file under `changes/` that is not a fragment, such as `README.md` or a file with a bad slug.
  - The check validates every fragment that the pull request adds or modifies against the schema, as it stands at the head commit, with or without the label.
  - The workflow runs `node scripts/release/check-fragments.mjs --base <base sha> --head <head sha> [--skip]`. The same command reproduces it locally, for example with `--base origin/main --head HEAD`.
- **Breaking-change detection,** also part of `changes`. `cargo-semver-checks` compares each workspace crate against the pull request's base commit, and the workflow regenerates and diffs the TypeScript SDK's API report. A reported breaking Rust change, or a removed or changed line in the API report, fails `changes` and names the item, unless the pull request adds a `breaking` fragment.
- **The `skip-changes` label.** With the label on the pull request, the required-fragment check passes, because the workflow passes `--skip`. Adding or removing the label runs the check again, because the workflow runs on `labeled` and `unlabeled`. The label never waives a detected breaking change. That still needs a `breaking` fragment, because `--require-breaking` ignores `--skip`. The label exists on the repository. It is meant for pull requests that change product paths but ship nothing to a user, such as a version raise or a lockfile refresh.

**Allowlist.** A pull request needs no fragment when every changed path matches one of these: `docs/**`, `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `NEXUS_MEMORY.md`, `LICENSE-*`, `.github/**`, `scripts/**`, `moon.yml`, `**/moon.yml`, `.moon/**`, `biome.json`, `tsconfig.base.json`, `.prototools`, `.nvmrc`, `.gitignore`, `**/tests/**`, `**/test/**`, `**/*.test.*`, `changes/**`, `release-notes/**`. Every other path ships to a user and needs a fragment or the label. That includes `Cargo.toml`, `Cargo.lock`, every `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `rust-toolchain.toml` and every `src/`. Lockfile updates need a fragment or the label too, including Dependabot's once it covers lockfiles.

**Lifecycle.** Fragments stay in place at alpha and beta. On `release/X.Y`, the last notes pull request before the release tag removes the release's fragments. On `main`, the release workflow's commit after the release removes them. Git history keeps them.

## Release notes

**Status.** The format below is the process. Several parts are active: the index file `release-notes/index.json`, its shape test in `repo:test`, `index.mjs add`, and the curated-notes rendering (`render-notes --beta` and `--release`). The publish job that runs them is built by `mt08.a4xv`, and the skeleton generator `draft-notes` by `mt08.4fp9`.

- **One Markdown file per release version**, `release-notes/<X.Y.Z>.md`. The same file serves from `beta.1` to the release. It is edited on `release/X.Y` before each beta, and the release archives it on `main`.
- **"Changes since beta N-1".** For a beta's GitHub notes, the publish job generates this section at publish time from the fragments added on the branch since the previous beta tag. The section is never committed. `render-notes --beta X.Y.Z --since <previous beta tag|none>` prints the curated file as it is at the commit. After a previous beta it adds a section `## Changes since <tag>` with those fragments, unfiltered, so a cycle fix appears there, each under `### <slug>`. `render-notes --release X.Y.Z` prints only the file.
- **`release-notes/index.json`** lists one entry per published beta and release, newest last, sorted ascending, with unique tags:

  ```json
  { "releases": [ { "version": "0.1.0", "stage": "beta", "tag": "v0.1.0-beta.1", "date": "YYYY-MM-DD", "path": "release-notes/0.1.0.md" } ] }
  ```

  Every entry has exactly these five keys.
  - `version` is a plain `X.Y.Z`.
  - `stage` is `beta` or `release`, and `tag` matches it: `vX.Y.Z` for a release, `vX.Y.Z-beta.N` for a beta. With `tag`, a consumer can fetch the file as it was at that tag.
  - `date` is the UTC calendar day of publishing, as `YYYY-MM-DD`: a real day, with no time and no time zone.
  - `path` is `release-notes/<version>.md`.

  The release workflow adds an entry with `node scripts/release/index.mjs add --version X.Y.Z --stage beta|release --tag <tag> --date <UTC day>` and commits the file to `main`. Adding an identical entry again changes nothing and prints `already indexed`. Adding an entry for an indexed tag that differs in any field fails. `validateIndex` in `scripts/release/index.mjs` holds the committed file to this shape in `repo:test`. Before the first beta the file is `{ "releases": [] }`.
- **Order of sections** in curated notes:
  1. a summary of the release, two or three sentences long;
  2. **Breaking changes**, each with problem, use and migration;
  3. **New features**, each with the problem and then the use;
  4. **Deprecations**;
  5. **Fixes**, each with symptom, when, cause and fix;
  6. **Security**;
  7. **Known issues**: bugs introduced in this cycle that we know of and have not fixed yet, each with a workaround if there is one.
- **The filter.** A `fix` with `introduced: cycle` never appears in curated notes, because a user of the previous release never saw that bug. It still appears in alpha notes and in "Changes since beta N-1". `internal` fragments never appear in curated notes.
- **Tone.** Write factually, for the user of the product. The notes may say why a feature is worth having, but they make no claim the product does not back. Keep each entry to one paragraph, plus a code block where it helps. The notes are in English only.
- **Where the notes appear.**
  - **GitHub.** The release body is the file's content. Betas after the first also carry the generated "Changes since beta N-1" section. Alpha prereleases carry the alpha notes instead (see [Alpha](#alpha)).
  - **CLI.** This is a contract for the `ncdb` crate, built when the CLI exists. `ncdb changes` prints the notes of the installed version. For an installed beta, it prints the curated file as it was at that beta's tag, found through `tag`. `ncdb changes <from>..` prints the curated notes of every release after `<from>`, read from `index.json` and the files on `main`, and never alpha fragments. The CLI may print a one-line hint when a newer release exists. It renders the Markdown for the terminal itself.
  - **Landing page.** This is a contract, built when the landing page exists. The landing page shows released versions only, rendered from `index.json` and the files on `main`, plus the roadmap from open milestones.

## Alpha

**Status.** Active. The `release` workflow's job `alpha` (`.github/workflows/release.yml`) runs `next-version` and `render-notes --alpha`. While the repository is private, an alpha is a GitHub prerelease that only collaborators can see, and nothing goes to a registry.

You do nothing by hand. Merge a product-changing pull request with its fragment to `main`, and the alpha follows once `ci` is green.

The `release` workflow runs on `workflow_run` of the `ci` workflow (`types: [completed]`, `branches: [main]`). The job `alpha` proceeds only for a run that meets all of these conditions: its `conclusion` is `success`, its `event` is `push`, its `head_branch` is `main`, and its `head_repository` is this repository. A run triggered by a pull request never qualifies, a fork's included, because the job holds `contents: write`. The workflow itself has only `contents: read`. The job runs in the concurrency group `release-alpha`, which never cancels a run in progress, for at most 20 minutes, and every step runs in `bash` with `pipefail`. The commit is the run's `head_sha` (`github.event.workflow_run.head_sha`, or `github.sha` on a dispatch). The job then works through these steps:

1. **Guard.** Before any checkout, the job reads the head of `main` with `git ls-remote --exit-code`, using the runner's own git and the job's token. If the commit is no longer that head, the job stops green with a notice and skips every later step. If `ls-remote` fails, the job fails. It never reads that failure as a reason to skip.
2. **Compute.** The job checks out the commit with full history and tags. It does not leave its token in the checkout's git configuration (`persist-credentials: false`); only the publishing step gets the token. It installs the toolchain and runs `node scripts/release/next-version.mjs --json` (see [Versioning](#versioning)). That prints the answer to the log and to `next.json` in the checkout. It commits nothing.
3. **Skip.** If next-version answers `"skip": true`, the job stops green. This happens when none of the fragments added since the previous alpha has a `type` other than `internal`. The previous alpha is the nearest alpha tag reachable from the commit. Before the first alpha, the job looks at every fragment present.
4. **Render.** The job renders the alpha notes with `node scripts/release/render-notes.mjs --alpha --since <previous alpha | none> --repo <owner/name>`. The notes list every fragment added since the previous alpha, in slug order. Each fragment gets a heading `## <slug>`, a line `*<type>* · components: … · tickets: …`, and its body with its sections demoted to `###`. The notes end with `Previous alpha: [<tag>](<its release>)` or `First alpha.` Alpha notes are raw, so they list `internal` fragments and `introduced: cycle` fixes too.
5. **Publish.** The job publishes a GitHub prerelease `v<next>-alpha.<n>` with the notes as its body, and the tag with it. `gh release create "$TAG" --target "$SHA" --prerelease` creates the tag on the commit and the release in one API call. There is no separate `git tag` or `git push`, so the job never leaves a tag without a release. If the step fails, it created nothing, and a re-run computes the same alpha again. If the release already exists because an earlier run succeeded, the step creates nothing. On such a re-run, next-version answers `skip` anyway, because the tag already sits on the commit. Once binaries are attached (`mt08.h4rh`), the job uploads them after the release exists, and that ticket must keep a re-run complete. CLI and relay binaries are attached once they exist. npm gets the alpha under the dist-tag `alpha` once the repository is public and the SDK is publishable (see [Not yet active](#not-yet-active)).

Alphas never go to crates.io. crates.io versions are permanent, and one version per merge would bury the real versions. Rust users who want an alpha depend on the git tag.

Sometimes no alpha appears. That is expected in three cases:

- **All-internal merges.** A merge whose fragments are all `internal`, or that needed no fragment, produces no alpha, because next-version answers `skip`.
- **The commit is no longer the head of `main`.** When two merges land in quick succession, the older one must not produce an alpha with a higher number on older code, so the guard skips it. The next alpha covers it, because the job computes alpha notes since the previous alpha tag, not per merge. Nothing is lost.
- **The workflow's own pushes to `main`.** The release workflow's commit after a beta or release starts no `ci` run, so no alpha follows it. While such a push is the head of `main`, the guard also skips the run of the merge before it. That merge's alpha is delayed by one run, and the alpha of the next product-changing merge covers it.

A tag pushed with the workflow's own `GITHUB_TOKEN` does not start another workflow run. The publish job (`mt08.a4xv`) also ignores `-alpha.` tags explicitly.

**Dry run.** A `workflow_dispatch` is always a dry run. With an empty `tag` input, it runs the alpha job:

```sh
gh workflow run release.yml --ref main
gh run watch "$(gh run list --workflow release.yml --event workflow_dispatch --limit 1 --json databaseId --jq '.[0].databaseId')"
git ls-remote --tags origin 'v*-alpha.*'   # unchanged: a dry run tags nothing
```

Its log shows the guard's verdict and the answer of next-version, which is the content of `next.json`: either `"skip": true` with the reason, or the version, the tag, the previous alpha and the fragments. When it does not skip, the log also shows the rendered notes. The dry run skips the tag-and-publish step (`DRY_RUN`), so it tags and publishes nothing. A dispatch on a ref other than `main` stops at the guard, because its commit is not the head of `main`, and computes nothing. Only a dispatch on `main` proves the job. A dispatch with a non-empty `tag` is the publish dry run of `mt08.a4xv`, and it does not run the alpha job.

**Reproducing it locally.** `node scripts/release/next-version.mjs [--json] [--head <ref>]` and `node scripts/release/render-notes.mjs --alpha --since <tag|none> --repo nocentraldb/nocentraldb [--head <ref>]` give the same answer for the same commit, as long as your local tags match `origin` (`git fetch --tags`). Both read every file as it stands at the commit, never from the working tree.

## Beta

**Status.** The steps below are the process. The publish job's scripts are active (`verify-tag`, `render-notes --beta`, `index.mjs`, `publishable`). The job itself is not yet active, built by `mt08.a4xv`, and `draft-notes` and `cut` are built by `mt08.4fp9`. Registry publishing waits for the repository to go public (`mt08.1213`).

1. **Propose (PM).** When the release epic is no longer blocked, the PM compares the epic's description with what was delivered. If the scope is met, the PM proposes a beta to the owner and attaches a draft of the release notes. If not, the PM says what is missing.
2. **Draft (PM or agent).** The PM or an agent drafts `release-notes/<X.Y.Z>.md` (see [Release notes](#release-notes)) from two sources. The first is every fragment since the previous release, which `draft-notes` renders into the skeleton with the filter applied and the sections in order. The second is every board item created or closed since the release epic was created, read with `nxf`; it matters most for Known issues.
3. **Pull request to `main`.** The draft goes into a pull request against `main`. The owner edits and approves it there. Merging it is the approval.
4. **Cut (`cut`).** After the merge, `release/X.Y` is cut from the merge commit (see [Branches](#branches)). The cut tool sets the manifests on the branch. It prints the push command, and the command for the pull request that raises `main` to `X.(Y+1).0` with the label `skip-changes`.
5. **Push the branch.** The owner pushes `release/X.Y`, or the PM does on the owner's word.
6. **Tag (owner).** The owner pushes the beta tag on the branch head:

   ```sh
   git tag vX.Y.Z-beta.1 && git push origin refs/tags/vX.Y.Z-beta.1
   ```

On the tag, the publish job takes over. The `release` workflow runs on `push` of tags `v*` and excludes `-alpha.` tags. It runs the full gate `pnpm gates` on the tagged commit itself, checks the tag against the manifests, writes the full version into the build checkout, and builds. It then publishes a GitHub prerelease whose body is the curated file. For betas after the first, it adds the generated section "Changes since beta N-1", which lists the fragments added on the branch between the two beta tags (`git diff --diff-filter=A --name-only <previous beta tag> <this tag> -- changes/`). Once the repository is public and the packages are publishable, it also publishes to npm under the dist-tag `beta`, and to crates.io as a prerelease version. Finally it appends the beta to `release-notes/index.json` on `main`.

If the publish job may not publish a tag, `publish` turns red before anything is built or published. The tag stays in place for the owner to delete or move. `scripts/release/verify-tag.mjs --tag <tag> --commit <sha>` holds the rules. It checks them in this order and stops at the first failure:

1. The tag is `vX.Y.Z` or `vX.Y.Z-beta.N`. This job never publishes alpha tags.
2. `X.Y.Z` is the workspace version in `Cargo.toml` at the commit.
3. The commit is reachable from `origin/release/X.Y`.
4. For a release only: a `vX.Y.Z-beta.N` tag exists, because every release passes through beta.
5. For a release only: the commit is the head of `origin/release/X.Y`.
6. For a release only: since the last beta, only `release-notes/` and `changes/` changed. The last beta is the highest `vX.Y.Z-beta.N` of the same `X.Y.Z`, compared numerically, so `beta.10` is above `beta.2`.
7. For a release only: `changes/` holds no fragment at the commit.
8. `release-notes/X.Y.Z.md` exists at the commit.

With `--json` it prints `{ "stage", "version", "lastBeta" }` for the job. `version` is the plain `X.Y.Z`. For a release, `lastBeta` is the beta that rule 6 compared against. For a beta, it is the previous beta of the same version, which is the `--since` of `render-notes --beta`, or `null`.

`scripts/release/publishable.mjs --json` tells the registry steps which packages to skip. An npm package is publishable when it is not `private` and its `exports` or `main` point under `dist/`. A crate is publishable when its `[package]` has no `publish` key, or has `publish = true`. Today neither `@nocentraldb/sdk` nor `ncdb-core` is publishable.

**Fixes during beta:**

- Land the fix on `main` first, with its fragment. On `main` it produces alphas of the next version.
- Cherry-pick it to `release/X.Y`, fragment included.
- The fragment carries `introduced: cycle` if the bug only existed since the last release, or `introduced: X.Y.Z` with the released version that introduced it.

**Later betas.** Before each beta tag, a pull request to the branch updates the curated file there. Then the owner pushes `vX.Y.Z-beta.2`, `vX.Y.Z-beta.3` and so on, on the branch head.

**Checks on the release branch.** Pull requests to `release/X.Y` run `ci` and `changes` exactly like pull requests to `main`, because `ci.yml:12` triggers on every `pull_request`. Only pushes to a release branch do not run `ci`. That is why the publish job runs the full gate on the tagged commit itself.

## Release

**Status.** The checklist below is the process. The publish job's scripts are active (`verify-tag`, `render-notes --release`, `index.mjs`, `shipped-fragments`, `publishable`). The publish job and its commit back to `main` are not yet active, built by `mt08.a4xv`. `promotion-check` is built by `mt08.4fp9`. The upgrade test waits for the relay (`mt08.nptg`, behind `mt08.7wjb`).

The PM checks the promotion conditions in its sessions and proposes the release to the owner when all of them hold. The owner decides.

| # | Condition | Who checks |
|---|---|---|
| 1 | **Soak.** The latest beta has been out for at least 7 days with no newer beta, according to the GitHub release dates read with `gh`. | `promotion-check` |
| 2 | **No open regressions.** No open bug of P0 or P1 has `introduced` in this cycle. Lower ones go under Known issues. | the PM, on the board |
| 3 | **Upgrade tested.** An automated test upgrades from the previous release to the candidate. It covers stored data, `model.lock.json`, an old client against the new relay and a new client against the old relay, and checks convergence throughout. | the upgrade test |
| 4 | **Same product code.** The release is tagged on the head of `release/X.Y`, and since the last beta tag only `release-notes/` and `changes/` have changed. | `promotion-check`; the publish job also refuses the tag otherwise |
| 5 | **Security.** `cargo audit` and `pnpm audit` report nothing, and no security advisory is open. A missing `cargo-audit` counts as a failed condition, never as a pass. | `promotion-check` |
| 6 | **Notes approved.** The owner approved the final curated notes, and every breaking change has a Migration section. | the PM, in the notes pull request |
| 7 | **Real use.** At least one of our own projects runs the beta, with no new bug report. | the PM, on the board |

Before 1.0, conditions 1, 2, 4, 5 and 6 apply from the first release. Conditions 3 and 7 apply as soon as there is a relay and a real user.

For condition 2, open bugs have no fragment yet. The PM therefore derives `introduced` from the bug ticket's description: when the bug was first seen, and which version or change it came with. If the description does not say, the PM asks and does not guess. The bug's fragment records the value when the fix lands.

Steps:

1. **Final notes pull request** to `release/X.Y`. It makes the curated notes final and deletes the release's fragments from `changes/`. The tooling refuses a release while `changes/` on the branch is not empty.
2. **Tag (owner).** On approval, the owner pushes `vX.Y.Z` on the head of `release/X.Y`:

   ```sh
   git tag vX.Y.Z && git push origin refs/tags/vX.Y.Z
   ```

3. **What the publish job does:**
   1. It runs the gate and `verify-tag`, with the eight rules listed under [Beta](#beta). The tag must match the manifests and sit on the head of `release/X.Y` after a beta. The diff rule of condition 4 must hold: `git diff --name-only <last beta tag> <release tag>` may show only `release-notes/` and `changes/`. No fragment may be left in `changes/`, and `release-notes/<X.Y.Z>.md` must exist at the tag. Any failure turns `publish` red before anything is built or published, and the tag stays in place for the owner to delete or move.
   2. It writes the version into the checkout and builds. The version is embedded, so the bytes differ from the beta's. Then it publishes to crates.io, to npm under `latest`, and as a GitHub release marked latest whose body is the file's content, without a manual step. It publishes to the registries once the repository is public and the packages are publishable.
   3. It commits back to `main`, in one commit that the workflow pushes. The commit contains the updated `release-notes/index.json`, the final `release-notes/<X.Y.Z>.md` as it stands at the tag, and the removal of every fragment that shipped. With the notes file on `main`, `main` becomes the archive that the landing page renders from. The shipped fragments are exactly the files deleted under `changes/` on the branch between the merge base with `main` and the release tag (`git log --diff-filter=D --name-only --format= <merge-base>..<tag> -- changes/`). `node scripts/release/shipped-fragments.mjs --tag vX.Y.Z` lists them with the content that shipped. With `--delete` on a checkout of `main`, it deletes a fragment only when the fragment's content is the blob that shipped. If a new fragment on `main` reused the slug, the script keeps it and names it (`kept <path>: differs from the shipped fragment (slug reused?)`). It never deletes a fragment by its name alone.
   4. It closes the GitHub milestone with `gh`. Once the landing page exists, it picks up the change from `main`.

   A push to `main` made with the workflow's token does not start the `ci` workflow. That commit touches only `release-notes/` and `changes/`, and the fragment schema check and the index test cover those paths on the next pull request.
4. **When something fails.** A partial publish, where the GitHub release exists but a registry step failed, leaves the job red and retracts nothing. Re-run it. Every publish step first checks whether its tag, release or package version already exists, and skips it if so. If the commit to `main` fails to push because `main` moved, the job fetches and retries once. If it still fails, the job is red, and someone has to make the commit by hand. `mt08.a4xv` adds the steps for that to this page.
5. **After the release.** The PM updates the `active-release` memory (see [Planning a release](#planning-a-release)).

**Patch releases** `X.Y.(Z+1)` go through the same branch and through beta again. The owner approves, the branch's manifests are set to `X.Y.(Z+1)`, `release-notes/X.Y.(Z+1).md` is drafted, and `vX.Y.(Z+1)-beta.1` is pushed on the branch head.

**Withdrawal.** If a published version is broken, we yank it on crates.io and deprecate it on npm, and a fixed version follows at once. Nothing is deleted.

## Planning a release

- **The release epic.** Each planned release has one epic on the board, for example "Release 0.3.0". Its description states what must be true for the release. Attach the epics and tickets the release needs as **dependencies**, never as children:

  ```sh
  nxf dep add <release-epic> <item>
  ```

  A dependency blocks, while a parent only groups. A deferred or closed parent would also hide its children.
- **The milestone mirror.** The release epic is mirrored publicly as a GitHub milestone, with the version as its title and the public part of the epic as its description. Once the landing page exists, the epic also appears on its roadmap. Until nexus-flow `6j6v.qv39` (GitHub Issues and milestone integration) exists, update the mirror by hand when the release is planned and whenever its scope changes. Each item's public visibility is decided per item, and the default is not public. Milestones work on the private repository too, and their visibility follows the repository's.

  ```sh
  # when the release is planned
  gh api repos/{owner}/{repo}/milestones -f title=<X.Y.Z> -f description=<text>
  # when its scope changes (<n> is the milestone's number)
  gh api -X PATCH repos/{owner}/{repo}/milestones/<n> -f description=<text>
  # at the release (the publish job does this once it exists)
  gh api -X PATCH repos/{owner}/{repo}/milestones/<n> -f state=closed
  ```

- **The `active-release` memory.** A project memory with the key `active-release` records the release in progress. Its body names the version, the release epic's id, the stage, and this page as the process. The stage is `planned`; or `beta`, with the branch and the latest beta tag; or `released`, with the date. Its introduction line names the version and the epic id.

  The PM writes the memory, because the PM plans releases and is the one that reads it. It writes it at three moments:

  1. when a release is planned, right after the release epic is created (stage `planned`);
  2. when `beta.1` is tagged (stage `beta`, with the branch and the tag), and again after each further beta tag;
  3. when the release ships. The PM rewrites the memory for the next planned release if one exists, and otherwise removes it with `nxm forget active-release`.

  The command:

  ```sh
  nxm remember "<version>, epic <id>, stage <planned|beta: release/X.Y, vX.Y.Z-beta.N|released YYYY-MM-DD>; process: docs/release-management.md" --introduction "Active release <version> (epic <id>): stage <stage>" --key active-release
  ```

  After every write, and after `nxm forget active-release`, commit the regenerated `NEXUS_MEMORY.md` at once, staging only that path:

  ```sh
  git add NEXUS_MEMORY.md && git commit -m "Record the active release <version>" -- NEXUS_MEMORY.md
  ```

  Make that commit on `main` or on the current feature branch, **never on a `release/*` branch**. A commit there would break the same-product-code rule of promotion condition 4. The PM makes exactly this commit and changes nothing else in the working tree.

  While no release is planned, no `active-release` memory exists.

## The PM's duties

The PM persona receives `nxm prime` and `nxf prime` at session start, so it sees the active release and whether its epic is still blocked. These checks run in agent sessions, never in GitHub Actions, because the board (`.nxs/`) is local and not synced, so CI cannot read it. The PM reads the facts that live on GitHub with `gh`: CI status, release dates and tags. Nothing is pushed to the owner, because `nxc` cannot reach a person. Once nexus-flow `6j6v.ek9g` gives `nxc` a way to notify the owner, the PM can tell the owner about a ready beta or release without waiting for the next session.

- **Surface the active release** at session start, from the `active-release` memory and the board.
- **Check the scope** when the release epic is no longer blocked. Raise it with the owner in the next session, compare the epic's description with what was delivered, and either propose a beta with a draft of the release notes or say what is missing.
- **Derive `introduced`** for open bugs from the bug ticket's description: when the bug was first seen, and which version or change it came with. Ask when the description does not say. Never guess.
- **Run `promotion-check`** for conditions 1, 4 and 5 (not yet active, built by `mt08.4fp9`), and check conditions 2, 6 and 7 on the board and in the notes pull request.
- **Propose** the beta and the release to the owner. The owner decides and pushes the tag.
- **Write the `active-release` memory** at the three moments above, and commit `NEXUS_MEMORY.md`, never on a `release/*` branch.
- **Mirror the release epic** as a milestone (see [Planning a release](#planning-a-release)).

## Supply chain and policies

1. **Provenance.** npm publishes with `--provenance`, and binaries get GitHub artifact attestations (`actions/attest-build-provenance`). Both need a public repository.
2. **Breaking-change detection.** On every pull request, `cargo-semver-checks` compares each workspace crate against the base commit, and the workflow regenerates and diffs the TypeScript SDK's API report, which is committed under `packages/sdk/etc/`. A reported breaking change fails `changes` unless the pull request adds a `breaking` fragment. This works on a private repository. It is not yet active, built by `mt08.nrt0`.
3. **No long-lived tokens.** CI publishes to npm and crates.io through Trusted Publishing, which uses OIDC from GitHub Actions, with `id-token: write` on the publish job only. The owner's passkey-protected accounts stay out of automation.
4. **Withdrawal.** If a published version is broken, we yank it on crates.io and deprecate it on npm, and a fixed version follows at once. Nothing is deleted.
5. **Security advisories.** We publish them through GitHub Security Advisories, with a Security section in the notes.
6. **Deprecation policy.** We mark anything we plan to remove as deprecated, and announce it in the notes, at least one release before we remove it.

**Published packages.** On crates.io: `ncdb-core` (core), `nocentraldb` (Rust SDK) and `ncdb` (the CLI, so that `cargo install ncdb` works). On npm: `@nocentraldb/sdk`. GitHub releases carry the CLI and relay binaries. All packages share one version. The publish steps skip a package that is not publishable (`private: true`, `publish = false`, or no package at all) and do not fail because of it.

**Going public.** The repository is private on the free GitHub plan today, and it goes public before the first real release, 0.1.0. On a private repository, only collaborators can see GitHub releases, milestones and binaries. npm provenance does not work there, and artifact attestations need Enterprise Cloud. Until the repository is public, alphas (and betas, if any) go to GitHub only, visible to collaborators, and nothing goes to the registries. Going public switches on the following (`mt08.1213`, `mt08.k1jh`, `mt08.may6`):

- Trusted Publishing for npm and crates.io, with `id-token: write`, `--provenance` and artifact attestations;
- a publishable `@nocentraldb/sdk` (build output in `dist/`, `exports`, no `private: true`) and a publishable `ncdb-core` (no `publish = false`);
- branch protection that requires the `gates` and `changes` checks, a tag ruleset for `v*`, and a bypass that lets the release workflow push its commit to `main`;
- the public milestone mirror.

**The owner's first publish.** Trusted Publishing can only be configured for a package that exists. The owner therefore publishes each package once by hand, before configuring it. Every later publish goes through CI.

## Tooling reference

| Unit | Does | Status |
|---|---|---|
| `scripts/release/lib/` (`cli.mjs`, `fragments.mjs`, `git.mjs`, `glob.mjs`, `test-repo.mjs`, `semver.mjs`, `manifests.mjs`) | shared command-line helpers; shared parsing of fragments, git, allowlist globs, SemVer and tags, and manifests; isolated git repositories for tests | active |
| `scripts/release/check-fragments.mjs [--dir <changes>]` | validates every `changes/*.md`; with `--base <rev> --head <rev> [--skip]`, also applies the allowlist and the required-fragment rule; `--require-breaking` ignores `--skip` | schema mode and `--base/--head/--skip` active; `--require-breaking` exists, but nothing passes it until the breaking-change detection (`mt08.nrt0`) |
| `scripts/release/check-version.mjs [--tag <v…>]` | checks that the manifests agree on a plain `X.Y.Z`, that inter-crate dependencies are pinned in `[workspace.dependencies]`, and that `Cargo.lock` follows; with `--tag`, checks that the tag matches the manifests for beta and release | active (gate task `repo:version`); `--tag` is active and the publish job uses it once it exists (`mt08.a4xv`) |
| `scripts/release/set-version.mjs <version> [--no-lock]` | writes the version into `Cargo.toml` `[workspace.package]`, pins the `[workspace.dependencies]` path entries to `=<version>`, sets every workspace `package.json`, then runs `cargo update --workspace --offline` | active; the publish job and the cut run it once they exist (`mt08.a4xv`, `mt08.4fp9`), and the alpha job once it builds binaries (`mt08.h4rh`) |
| `scripts/release/next-version.mjs [--json] [--head <ref>] [--index <file>]` | computes the next alpha tag from the tags, the fragments and the workspace version at the commit and from `release-notes/index.json`, or answers `skip` with its reason | active (alpha job) |
| `scripts/release/render-notes.mjs --alpha --since <tag\|none> --repo <owner/name> \| --beta <X.Y.Z> --since <tag\|none> \| --release <X.Y.Z> [--head <ref>]` | prints the GitHub release body: the alpha notes; the curated file plus "Changes since" the previous beta; or the curated file only | active; the alpha job runs `--alpha`, and the publish job runs `--beta` and `--release` once it exists (`mt08.a4xv`) |
| `scripts/release/verify-tag.mjs --tag <tag> --commit <sha> [--remote <name>] [--json]` | applies the refusal rules for beta and release tags (see [Beta](#beta)) | active; the publish job runs it once it exists (`mt08.a4xv`) |
| `scripts/release/index.mjs add --version --stage --tag --date [--file]` | appends to `release-notes/index.json`, sorted and idempotent; exports `validateIndex` | active; the publish job runs it once it exists (`mt08.a4xv`) |
| `scripts/release/shipped-fragments.mjs --tag <vX.Y.Z> [--main <ref>] [--delete]` | lists the fragments that shipped, with their blobs; `--delete` removes those on `main` whose content is that blob | active; the commit back to `main` runs it once it exists (`mt08.a4xv`) |
| `scripts/release/publishable.mjs [--json]` | lists which packages the publish steps may publish, each with its reason | active; the publish job runs it once it exists (`mt08.a4xv`) |
| `scripts/release/commit-back.mjs` | makes the release workflow's commit to `main` | not yet active (`mt08.a4xv`) |
| `scripts/release/draft-notes.mjs <X.Y.Z>` | writes the curated-notes skeleton, with the filter applied and the sections in order | not yet active (`mt08.4fp9`) |
| `scripts/release/cut.mjs <X.Y.Z>` | creates `release/X.Y`, sets the manifests, and prints the push, raise-PR and beta-tag commands | not yet active (`mt08.4fp9`) |
| `scripts/release/promotion-check.mjs <X.Y.Z>` | reports promotion conditions 1, 4 and 5 with evidence | not yet active (`mt08.4fp9`) |
| `scripts/release/api-report.mjs`, `packages/sdk/etc/` | the TypeScript SDK's API report | not yet active (`mt08.nrt0`) |
| moon task `repo:fragments` | runs `node scripts/release/check-fragments.mjs` in `pnpm gates` | active |
| moon task `repo:version` | runs `node scripts/release/check-version.mjs` in `pnpm gates` | active |
| moon task `repo:api-report` | runs the API report in `pnpm gates` | not yet active (`mt08.nrt0`) |
| moon task `repo:test`, widened | also runs the tests under `scripts/release/` (`node --test 'scripts/*.test.mjs' 'scripts/release/**/*.test.mjs'`) | active |
| workflow `changes.yml` (check `changes`) | required fragment, `cargo-semver-checks`, API report diff | required fragment active; `cargo-semver-checks` and the API report diff not yet active (`mt08.nrt0`) |
| workflow `release.yml`, job `alpha` | publishes an alpha after a green `ci` run of a push to `main`; tagless dispatch dry run | active |
| workflow `release.yml`, jobs `verify`, `build`, `publish`, `commit-back` | beta and release on `v*` tags that are not alphas; dispatch dry run | not yet active (`mt08.a4xv`) |
| `scripts/changes-workflow.test.mjs`, `scripts/release-workflow.test.mjs`, `scripts/lib/workflow-checks.mjs` | the workflows' contracts, with mutants | active; `release-workflow.test.mjs` covers the job `alpha`, and the publish jobs join it with `mt08.a4xv` |
| label `skip-changes` | waives the required fragment, never a detected breaking change | active |
| `changes/README.md` | the writer's guide for fragments | active |
| `release-notes/index.json` and its test | the notes archive index, held to its shape by `scripts/release/index.test.mjs` in `repo:test` | active |
| memory `active-release` | the release in progress, for the PM | the PM writes it when a release is planned |

## Not yet active

These parts wait on something outside this repository's tooling. Each one has a `WAIT:` chore on the board (`nxf show <id>`), and the tickets that need it depend on that chore.

- `mt08.1ny6`, WAIT: repository goes public. After that: Trusted Publishing, provenance, attestations, and a publishable SDK and core crate (`mt08.1213`); rulesets for `main` and tags, and the release workflow's bypass (`mt08.k1jh`, after `mt08.may6`).
- `mt08.tjwz`, WAIT: the ncdb CLI exists. After that: `ncdb changes`, CLI binaries at every stage, and `cargo install ncdb` (`mt08.h4rh`).
- `mt08.7wjb`, WAIT: the relay exists. After that: relay binaries on releases, and the automated upgrade test of promotion condition 3 (`mt08.nptg`).
- `mt08.mn1j`, WAIT: the landing page exists. After that: the roadmap from milestones, and the release notes from `index.json` and `main` (`mt08.zxg9`).

The Rust SDK needs no WAIT chore. The publish job publishes every publishable workspace crate, so `nocentraldb` ships once it exists and is publishable.
