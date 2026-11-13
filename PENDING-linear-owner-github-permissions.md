# Pending Linear ticket (Linear did not connect on 2026-11-13; create it as soon as it does)

Team: NOC. Label: needs-owner. Assignee: Carsten.

## Title

Owner setup: GitHub permissions that enforce the owner's approval on deployed code (decision B)

## Description

Decision B (Carsten, 2026-11-13): the agents act on GitHub as their own account, `nexusflow-ug`, so GitHub can tell the owner's approval from theirs. claude.ai is already connected to that account (checked: `get_me` answers `nexusflow-ug`). What is still missing is the part only the owner can set up in the repository's settings. The full runbook is `docs/relay-deploy.md`, sections 1.2 to 1.4, in #299; this ticket lists only the GitHub permissions.

### When

1. Merge #299 first. It brings `.github/CODEOWNERS`; the ruleset's Code Owners rule has nothing to enforce before it.
2. Then do steps A to C, in this order.

### A. The agents' account: Write on this repository only

Settings → Collaborators and teams on `nocentraldb/nocentraldb`:

- `nexusflow-ug` has the role **Write**. Not Maintain, not Admin, and no organization owner role.
- `nexusflow-ug` has **no access** to `nocentraldb/aws-deploy`.
- The Claude GitHub App stays installed on `nocentraldb/nocentraldb`.

With Write it pushes branches, opens PRs and merges PRs the rules allow, but cannot change rulesets, environments or their reviewers.

### B. Ruleset on `main`

Settings → Rules → Rulesets → New branch ruleset:

- Name: `main` (any name works). Enforcement: **Active**. Target: the default branch.
- **Require a pull request before merging**, with:
  - Required approvals: **0** (the Code Owners rule asks for your approval on owned paths by itself; PRs outside them stay mergeable by pm under the merge rule in `factory/CONFIG.md`);
  - **Require review from Code Owners**: on;
  - **Dismiss stale pull request approvals when new commits are pushed**: on.
- **Bypass list: empty.** Never add `nexusflow-ug`.

Effect: a PR that touches a path in `.github/CODEOWNERS` (infra/aws, ncdb-core, ncdb-plugin, the relay crates, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `pnpm-lock.yaml`, the deploy workflow, CODEOWNERS itself) merges only after `cabcookie` approved its latest commit.

### C. Environment `aws-prod`

Settings → Environments → New environment `aws-prod`:

- Deployment branches and tags: **only `main`**.
- Required reviewers: **`cabcookie` only**. "Prevent self-review": off.
- Variables (not secrets): `AWS_DEPLOY_ROLE_ARN`, `RELAY_ID`, `ALLOWED_ACCOUNTS`, `ALARM_EMAIL`, as `docs/relay-deploy.md` section 1.3 describes. These can wait until the `aws-deploy` setup is done; without `AWS_DEPLOY_ROLE_ARN` the deploy job skips and stays green.

### Check

- The next agent PR that touches an owned path shows "Review required" from a code owner, and pm cannot merge it.
- An agent PR that touches no owned path merges as before.
- `nexusflow-ug` cannot open Settings → Rules or Environments.

### Your decision (one point, not covered by decision B)

GitHub never lets a PR's author approve their own PR. With the bypass list empty, a PR **you** author that touches an owned path cannot merge unless someone else approves it.

- **A. Keep the bypass list empty** (recommended). You open no PRs on owned paths yourself; an agent opens them and you approve. Cost: none today, since agents write all changes.
- **B. Add the role "Repository admin" to the bypass list, mode "For pull requests only".** You can merge your own PRs on owned paths. Cost: an admin can then merge any PR past the rule; safe only as long as `nexusflow-ug` never gets Admin.

### Follow-up for pm

Once this is active, the factory's merge rule (`factory/CONFIG.md`) is brought in line: a PR touching a CODEOWNERS path needs the owner's approving GitHub review; pm requests it and never merges such a PR without it. That goes to the owner as its own PR (handover 2026-11-13, item 4).
