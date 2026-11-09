# CONFIG.md

The values of this repository's factory. FACTORY.md is copied unchanged between repositories; this file is the one that changes.

| Setting                            | Value                                                                                                                                     |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Owner (the source)                 | Carsten                                                                                                                                   |
| Product file                       | `factory/GENESIS.md`                                                                                                                      |
| Decision log                       | `factory/decisions/`, one file per decision; read it with `pnpm decisions` (`--help` lists the options)                                   |
| Tracker (the sink)                 | Linear, team `NOC`, through the Linear connector                                                                                          |
| Ticket id in commits and PR titles | `NOC-<number>`                                                                                                                            |
| Initiative for the daily note      | `NoCentralDB`                                                                                                                             |
| Label for questions to the owner   | `needs-owner`                                                                                                                             |
| Coding agents in parallel          | 4                                                                                                                                         |
| A lane is free again               | when pm has reviewed the PR and hands it to the owner; a PR that waits only for the owner holds no lane                                   |
| Expedite lanes                     | 1                                                                                                                                         |
| Base branch                        | `main`                                                                                                                                    |
| Branch per ticket                  | Linear's branch name for the ticket                                                                                                       |
| Gate command                       | `pnpm gates`, green before a PR is marked ready                                                                                           |
| Change fragments                   | A product-changing PR adds one under `changes/` (see `changes/README.md`)                                                                 |
| Largest code or test file          | 700 lines. `pnpm gates` fails on a longer file (`repo:file-size`); split it by concern (see FACTORY.md)                                   |
| Merge rule                         | Every PR opens as a draft. The owner ticks its Authorship box (`.github/AI_POLICY.md`); only then pm marks it ready and squash-merges it. Exception: a PR that only implements a decided RFC, as AI_POLICY.md defines it, pm checks, ticks "Agent, checked against a decided design", and merges without the owner. A second exception: the PR that carries a beta's release notes, curated by an agent, which pm checks, ticks "Agent, beta release notes" and merges without the owner. The notes of a stable release stay with the owner. |
| Daily note                         | Every day at 06:00 Europe/Berlin                                                                                                          |
| Runtime                            | Claude Code cloud environment `nocentraldb`                                                                                               |
| Cloud access                       | AWS account NoCentralDb-Prod, read-only, through an SSO login the owner confirms per session                                              |
| Language                           | English for everything durable; chat follows the owner's language                                                                         |
