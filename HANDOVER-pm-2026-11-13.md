# pm handover, 2026-11-13 ~13:50 UTC

The previous pm session (session_01QSAQLrzS8Mwz2aPRKfmrhF) ended because its GitHub tools answered "invalid session" after the owner switched claude.ai's GitHub connection to the agents' own account. This file is the state to resume from. It lives only on this branch, never on main.

Read first: factory/roles/pm.md, factory/FACTORY.md, factory/CONFIG.md. Chat with the owner in German; every artifact in English.

## First actions for the new pm session

1. Check GitHub works: `mcp__github__get_me` should show the agents' account (`nxsflow` or `nexusflow-ug`), not `cabcookie`. The repo API should show push true, admin false.
2. Linear was down at handover. Once it answers:
   - Create the **owner decision ticket** (it was never created, Linear timed out). Mark it Done: "Agents act as cabcookie on GitHub, so GitHub cannot require the owner's approval for AWS deploys". Decided (Carsten, 2026-11-13): **B**, agents get their own GitHub identity (done: claude.ai is now connected to the agents' account). CODEOWNERS, a ruleset on main (require PR, require Code Owner review, dismiss stale approvals, no bypass for agents, required approvals 0) and the `aws-prod` required reviewer `cabcookie` enforce the owner's approval for code that deploys to AWS. Lockfiles stay in CODEOWNERS (cost: every dependency change needs his review).
   - Create a **docs ticket** for rT1G's getting-started guide: "Docs: a getting-started guide for the Rust SDK, with snippets that compile". Send rT1G the number; his PR waits for it.
   - Record on NOC-295 that it still waits for the owner (remove the unused `tokens` table, recommended A, before the first deploy).
3. Merge **#305** (NOC-299, test-only, green on 432463a, internal fragment) as "checked against a decided design". Then set NOC-299 Done.
4. Bring the factory merge rule in line with decision B: a PR touching a CODEOWNERS path needs the owner's approving GitHub review; pm requests it and never merges such a PR without it. This changes factory/CONFIG.md, so it goes to the owner as its own PR.

## Agents (send_message via claude-code-remote)

| Agent | Session | Doing | Waiting for |
| --- | --- | --- | --- |
| Noka | session_01QSAQLrhuxGmN98KN54Noka | #302 NOC-268 (lose a device), #304 NOC-281 (npm alphas) | owner ticks |
| v9jp | session_01QSAQLv9jp4DPqZZiRhM4Pf | #305 NOC-299; linking branch carstenkoch/noc-267-linking (NOC-267/269, NOC-291 A) | #302 merge, then open linking PR |
| uWo4 | session_01QSAQLuWo4nztevkRqsNkbs | #299 NOC-272 PR 3 (deploy, reworked for option B, head 1b1ef98, CI green); NOC-290 sweep local | owner tick on #299; aws-deploy files ready (policies, test, handler list) for the owner to place in nocentraldb/aws-deploy |
| rT1G | session_01QSAQLrT1GhKiduH2rDrZqd | #303 S2 (queries, subscriptions); local branches: S3 (db.changes), tasks-ts (NOC-297), S4a (sync/status), NOC-177 (tabs), getting-started docs | #303 tick; NOC-298 answer for S4b |

Agent sessions may also need a restart if their GitHub pushes fail after the account switch.

## Open PRs waiting for the owner's tick (all green)

- **#299** NOC-272 PR 3: the deploy workflow, CODEOWNERS (including ncdb-core, ncdb-plugin, relay crates, infra/aws, lockfiles), and a runbook with the owner's four setup steps. Merging it before the setup deploys nothing. After merging, the owner turns on the ruleset and `aws-prod`.
- **#302** NOC-268: remove a lost device. New formats: the record-id labels `nocentraldb/account-statement` and `nocentraldb/account-wrapped-key`, the `wrappedKeys.sealed` layout, and the proposed `Removed.others` field. Decision file pending tick: needs "(Carsten, date, on #302)" text-only fix before merge.
- **#303** NOC-176 S2: queries and subscriptions in the TS SDK. A wire-format change in the bridge, so the owner ticks it. Has a `breaking` fragment.
- **#304** NOC-281: alphas to npm with Trusted Publishing. The owner first publishes `@nocentraldb/sdk@0.0.0` by hand and configures the Trusted Publisher (org nocentraldb, repo nocentraldb, workflow release.yml); the steps are in its docs.

After a tick: when the PR has "pending tick" decision files or "proposed" RFC marks, have the agent push a text-only commit dating them, check that the diff against the ticked head is text only, bring the PR up to date with main if main moved in overlapping files, then squash-merge with the pm note in the commit message.

## Open owner questions (needs-owner)

- **NOC-295:** remove the unused AWS `tokens` table before the first deploy (A recommended).
- **NOC-298:** a text form for `MemberId`, 64 lowercase hex like ObjectId (A recommended). S4b (members in TS) waits.
- NOC-131 (announcement), not urgent.

## Merge order and chains

- Accounts: #302 → v9jp's linking PR.
- TS SDK: #303 → S3 → tasks-ts (NOC-297) → S4a → NOC-177; S4b after NOC-298. Every one of these goes to the owner (wire format or workflow).
- AWS: #299 → NOC-290 (sweep, plus a timeout alarm) → NOC-295's removal if he picks A.

## Done today (since the 05:00 daily note)

Merged: #288 (owner), #296, #298, #291 (owner), #290 (owner), #292, #300, #301, #293 (owner), #289, #287, #286, #284 (owner), #285 (owner), #283, #282, #281 (owner). Owner decisions recorded: NOC-282/286 (guest space A1), NOC-285, NOC-288, NOC-289, NOC-291, NOC-292, CDK flags set in PR 3, deploy design B.

## Daily note

Next one 2026-11-14 06:00 Berlin, by the existing trigger trig_01JZqmrybFyHj4M2Ftni5wUi; the hourly tick is trig_01FF6SqDS7R6NKrVEcA5V7da. Both fire into the old session; re-point them (update_trigger with persistent_session_id) or recreate them for the new session.

## Status in the new pm session (session_01QSAQLsQvXgmRee4xCmszsT), 2026-11-13 ~14:05 UTC

- GitHub runs as `nexusflow-ug`.
- #305 merged as "checked against a decided design" (squash 3c0c4ce).
- pm triggers recreated for this session: hourly tick `trig_01SU2e5pXF42t5sozxKPJyhq`, daily note `trig_0138s2nw5JewokGFZapnLaYm`. The old ones are deleted.
- Noka, v9jp, uWo4 and rT1G were told to report to this session.
- Still open, because Linear did not connect: the option-B decision ticket (Done), the rT1G docs ticket (send rT1G its number), the NOC-295 note, NOC-299 Done.
- Linear done by a helper session (2026-11-13 ~16:00 UTC): NOC-300 (owner's GitHub permissions, needs-owner), NOC-301 (option B, Done), NOC-302 (rT1G's getting-started docs, In Progress), comment on NOC-295, NOC-299 Done.
