---
type: internal
tickets: [NOC-272]
components: [relay]
---
A merge to `main` that changes the relay deploys it to AWS through GitHub Actions and OIDC; docs/relay-deploy.md is the owner's runbook for the one-time setup, the first deploy, the smoke test and rollback. The account's role setup lives in the private repository nocentraldb/aws-deploy, and every change to a deployed path needs the owner's review (.github/CODEOWNERS).
