# 2026-11-13 · NOC-301 · A PR on a CODEOWNERS path needs the owner's approving review

- Decided by: Carsten
- Why: agents now act on GitHub as their own account (option B), so GitHub can enforce the owner's approval on the code that deploys to AWS; the factory's merge rule follows what GitHub enforces, and no exception lets pm merge such a PR without that review.
- Instead of: keeping the Authorship tick as the only owner approval, which GitHub cannot enforce, and letting pm merge decided-design, docs or tooling PRs on CODEOWNERS paths without the owner.
- Topics: factory, merge-rule, github, aws
