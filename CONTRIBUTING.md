# Contributing to NoCentralDB

Thank you for considering it. NoCentralDB is designed in the open and is young: nothing is released yet, and the design in [RFC 0001](docs/rfc/0001-nocentraldb.md) is still cheap to change. We prefer a few careful contributions to many quick ones.

## What we ask for

1. **Feedback on the RFCs.** This is the contribution we want most right now. Tell us about your app, your situation and where the design would break for you, in the repository's RFC discussions (GitHub Discussions, category *RFC feedback*), as [RFC 0001](docs/rfc/0001-nocentraldb.md#how-to-give-advice) describes. We read every reply, answer within a week, and publish a summary of what changed in the next draft. To change the text of an RFC itself, open a pull request against its file under `docs/rfc/`.
2. **Bug reports with a reproduction**, through the *Bug report* issue form. The most useful ones name the RFC promise the software breaks (the RFC number and section) and say whether you would help fix it. Links to your merged work in other open-source projects help us judge a report quickly; they are never required.
3. **Offers to take a milestone** from the RFC roadmap, through the *Offer to take a milestone* issue form. A milestone starts as a conversation about the approach, not as a pull request.
4. **Pull requests for issues labelled `help wanted`** (or `good first issue`). These issues carry written acceptance criteria; your pull request meets them.

Anything else starts as an issue or a discussion first. A pull request without one is closed, typo fixes excepted. Questions and ideas go to GitHub Discussions. Security problems never go into a public issue: see [SECURITY.md](.github/SECURITY.md).

## Design and behaviour changes

A change to the design or to documented behaviour starts as an issue or as an RFC under `docs/rfc/`, never in a pull request. An issue labelled `needs-design` is closed to pull requests until its design is agreed in the issue.

## Labels

| Label | Meaning |
| --- | --- |
| `needs triage` | New, not yet read by a maintainer. The bug report form sets it. |
| `help wanted` | Open for an outside pull request; the issue has written acceptance criteria. |
| `good first issue` | Like `help wanted`, and small enough to be a first contribution. |
| `needs-design` | Closed to pull requests until the design is agreed. |
| `milestone-offer` | An offer to take a roadmap milestone. The milestone form sets it. |
| `security-review` | Touches sync, the relay, auth, storage or the on-disk format, and gets a security review before merging. |
| `skip-changes` | Passes the `changes` check without a fragment; see below. |

## Before a pull request

Set up the tools as described in the [README](README.md#prerequisites), then run the project gate and make sure it is green:

```sh
pnpm install
pnpm gates
```

`pnpm gates` runs every check uncached. `pnpm gates:dev` is faster during development but replays cached results, so it does not replace `pnpm gates`. See [Gates](README.md#gates) for the individual checks and how to re-run a single test.

**Tests.** Every fix comes with a test that fails without the fix. Every new behaviour comes with a test that shows it. In the pull request, "How it was tested" lists only what you actually ran and saw.

**Versions.** Do not change a version in a manifest: everything the project publishes shares one version, and releases set it. A new crate says `version.workspace = true`; a dependency on another workspace crate is declared once in `[workspace.dependencies]` of `Cargo.toml`, as `name = { path = "crates/name", version = "=X.Y.Z" }` with the workspace version, and used as `name.workspace = true`; a new package under `packages/` or `apps/` carries the workspace version. The gate task `repo:version` fails otherwise; the rules are in [Versioning](docs/release-management.md#versioning).

**Change fragments.** A pull request that changes the product, meaning anything a user could notice in a crate, a package or an app, adds one fragment: `changes/<slug>.md`. Documentation, `.github/`, `scripts/`, tests and repository config need none; the exact allowlist is in [Writing a change fragment](docs/release-management.md#writing-a-change-fragment). A fix, for example, as `changes/sync-retry-after-timeout.md`:

```md
---
type: fix
tickets: [mt08.1qaz]
introduced: cycle
components: [core]
---
## Symptom
Sync stopped for good after a relay timeout.

## When
Only when the relay answered later than 30 seconds.

## Cause
The retry timer was cleared together with the connection.

## Fix
The timer is now owned by the sync loop and survives a reconnect.
```

The other types and their sections are in the writer's guide [changes/README.md](changes/README.md). `pnpm gates` checks every fragment against the schema, and on the pull request the `changes` check fails when a path outside the allowlist changed and no fragment was added; it names each such path. You may not know the right ticket id or type: for an outside pull request a maintainer, or an agent acting for them, may write or fix the fragment instead of sending the pull request back.

The label `skip-changes` passes the `changes` check without a fragment. It is legitimate only when the pull request changes product paths but ships nothing a user would notice: a version raise or a lockfile refresh. Anything a user could notice (a feature, a fix, a changed dependency that alters behaviour, a deprecation) needs a fragment, with `type: internal` when it is not worth a line in the curated notes. The label never waives a detected breaking change. Adding or removing the label re-runs the check.

**The SDK's API report.** `packages/sdk/etc/api-report.d.ts` is the TypeScript SDK's public API, generated from `packages/sdk/src/` by `tsc`. When a change alters the SDK's exports or their types, regenerate it and commit it with the change:

```sh
node scripts/release/api-report.mjs --write
```

`pnpm gates` fails while the committed report is out of date. On the pull request, the `changes` check compares the report with the one on the base branch: a removed or changed line is a breaking change, and so is a breaking change `cargo-semver-checks` finds in a crate. Either one needs a fragment of type `breaking`; additions need none beyond the usual fragment. To see the verdict before you push, run `node scripts/release/api-report.mjs --base origin/main`, which exits 3 and names each removed or changed line.

Once a pull request with a fragment other than `internal` is merged and `ci` is green on `main`, the `release` workflow turns it into an alpha: a tag `vX.Y.Z-alpha.N` and a GitHub prerelease whose notes are the fragments added since the previous alpha ([Alpha](docs/release-management.md#alpha)). How fragments become release notes, and how releases are made, is in [docs/release-management.md](docs/release-management.md).

**The pull request.** Fill in every section of the template, Authorship and `Follow-up:` included.

## Review

A small team of maintainers reviews everything: issues, discussions and pull requests. There is no fixed response time for issues and pull requests, and some will wait. RFC feedback is the exception: it is answered within a week, as the RFC promises. A pull request that follows this page, with an issue, a test, a fragment and a filled-in template, is the quickest one to review.

## AI, security and conduct

- [AI policy](.github/AI_POLICY.md): agents are welcome; a named human understands the change and answers review.
- [Security policy](.github/SECURITY.md): report vulnerabilities privately, never in public.
- [Code of conduct](.github/CODE_OF_CONDUCT.md): adapted from the Contributor Covenant 3.0; reports go to conduct@nocentraldb.io.

## Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as below, without any additional terms or conditions:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The license expression in every manifest is `MIT OR Apache-2.0`.
