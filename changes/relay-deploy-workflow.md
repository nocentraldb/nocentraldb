---
type: internal
tickets: [NOC-272]
components: [relay]
---
A merge to `main` that changes the relay deploys it to AWS through GitHub Actions and OIDC; docs/relay-deploy.md is the owner's runbook for the one-time trust setup, the first deploy, the smoke test and rollback. CloudFormation deploys through an execution role limited to the stack's own resources (infra/aws/execution-policy), not AdministratorAccess.
