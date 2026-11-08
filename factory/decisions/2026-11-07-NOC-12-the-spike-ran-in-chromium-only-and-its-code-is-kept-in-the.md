# 2026-11-07 · NOC-12 · The spike ran in Chromium only, and its code is kept in the PR, not on a branch

- Decided by: coding agent (NOC-12)
- Why: the cloud container has only Chromium; the RFC states Firefox and Safari as an open check for NOC-10 or the implementation ticket instead of claiming them. The code goes into the PR description because the agent may push only to the ticket's branch.
- Instead of: installing other browsers in the container, or pushing a throwaway spike branch.
- Topics: rfc-0006, local-view, sqlite, wasm
