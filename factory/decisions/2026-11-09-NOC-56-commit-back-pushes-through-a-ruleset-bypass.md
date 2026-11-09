# 2026-11-09 · NOC-56 · commit-back pushes to main through the ruleset's bypass for the GitHub Actions app

- Decided by: pm, on the coding agent's recommendation; the bypass itself is an owner setting
- Why: the ruleset "Require PR" on `main` refuses a direct push. A bypass for the GitHub Actions app keeps the release commit a direct push with the job's own token: no new credential, no extra pull request per beta or release waiting for the owner's review, and the design `docs/release-management.md` already described. The workflow does not change; `scripts/release-ruleset.test.mjs` holds the job to pushing with `github.token`, and the runbook gives the owner the exact settings step. The bypass reaches every workflow token with `contents: write`, which the runbook states.
- Instead of: commit-back opening a pull request, which needs the owner to allow GitHub Actions to create pull requests, and whose pull request, opened with the job's token, would never run the required `gates` and `changes` checks; or a deploy key as the bypass actor, a long-lived secret against "No long-lived tokens".
- Topics: release, ci, github, ruleset
