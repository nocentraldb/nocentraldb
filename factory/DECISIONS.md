# DECISIONS.md — the decision log

Every call that shapes the product or the code and is not obvious from the diff, newest at the end. Agents append; nobody rewrites an entry. The format is in `factory/FACTORY.md`.

### 2026-11-07 · no ticket · Tickets live in Linear, team NOC

- Decided by: Carsten
- Why: the factory needs a tracker that outlives every session; the cloud sessions reach Linear through its connector.
- Instead of: nexus-flow's local board (`nxf`), which does not survive an ephemeral cloud container.

### 2026-11-07 · no ticket · The first relay backend runs on AWS

- Decided by: Carsten
- Why: serverless on DynamoDB with S3, in the NoCentralDb-Prod account.
- Instead of: Postgres or SQLite first; both follow later.

### 2026-11-07 · no ticket · The first example app is the agent task board

- Decided by: Carsten
- Why: each ticket's body is a rich-text field, so the first example covers spaces, views, computed fields and focused text edits.
- Instead of: customer records with erasure, the proposal editor, or agent chat first.

### 2026-11-07 · no ticket · The factory runs 4 coding agents plus 1 expedite lane

- Decided by: Carsten
- Why: enough parallel work for one owner to review; an urgent ticket gets its own lane without reshuffling the others.
- Instead of: sizing the drain only by the token quota.

### 2026-11-07 · no ticket · Every merge waits for the owner

- Decided by: Carsten
- Why: the owner reviews every PR and ticks its Authorship box (`.github/AI_POLICY.md`); the factory runs autonomously up to a ready PR.
- Instead of: pm merging on its own after review.

### 2026-11-07 · no ticket · The factory lives in `factory/`, apart from the product docs

- Decided by: Carsten
- Why: GENESIS.md describes the product; FACTORY.md is generic and copies to another repository unchanged, with CONFIG.md holding this repository's values.
- Instead of: one GENESIS.md that mixes product and process, at the repository root.

### 2026-11-07 · no ticket · The daily note is a status update on the Linear initiative

- Decided by: Carsten
- Why: every day at 06:00 Europe/Berlin, with the initiative's health; questions to the owner become tickets labelled `needs-owner`.
- Instead of: a Linear document or a comment, where a question can get lost.

### 2026-11-07 · no ticket · Coding agents work test first

- Decided by: Carsten
- Why: every acceptance criterion gets a test that failed before its code existed; the test commit comes first, so pm can check the red step in the history.
- Instead of: tests written after the code, which can pass without ever having caught anything.

### 2026-11-07 · NOC-6 to NOC-13 · Owner-reserved designs go through an RFC PR before any code

- Decided by: pm
- Why: GENESIS.md reserves crypto, key handling, wire and on-disk formats, merge rules and public API names for the owner; an RFC as a draft PR gives the owner one place to decide, and the merge is the sign-off.
- Instead of: coding agents choosing formats and primitives inside implementation PRs, or pm asking each detail as a separate `needs-owner` ticket.

### 2026-11-07 · NOC-9, NOC-10 · The convergence harness and the wasm32 gate come before any field type

- Decided by: pm
- Why: they are the only core work that needs no owner decision, and every later field type is then built test first against a fixed definition of "converges" on both targets (principles 2 and 7).
- Instead of: starting with field types and adding the harness and the wasm build afterwards.

### 2026-11-07 · NOC-14 to NOC-17 · Implementation tickets wait in Backlog until their RFC is merged

- Decided by: pm
- Why: their criteria depend on the merged RFC; pm sharpens them against it and moves them to Todo then.
- Instead of: writing detailed criteria now that the RFCs may contradict.

### 2026-11-07 · NOC-5 · The daily note goes to the "Core foundation" project until initiatives are enabled

- Decided by: pm
- Why: initiatives are not enabled in the Linear workspace and the connector cannot create one; the note must not get lost while NOC-5 waits for the owner.
- Instead of: skipping the daily note until the owner answers.

### 2026-11-07 · NOC-5 · The daily note goes to the initiative NoCentralDB

- Decided by: Carsten
- Why: initiatives are enabled and the initiative exists; both projects are linked to it, as CONFIG.md says.
- Instead of: the interim note on the "Core foundation" project (reverses pm's entry "The daily note goes to the "Core foundation" project until initiatives are enabled").

### 2026-11-07 · NOC-12 · The spike ran in Chromium only, and its code is kept in the PR, not on a branch

- Decided by: coding agent (NOC-12)
- Why: the cloud container has only Chromium; the RFC states Firefox and Safari as an open check for NOC-10 or the implementation ticket instead of claiming them. The code goes into the PR description because the agent may push only to the ticket's branch.
- Instead of: installing other browsers in the container, or pushing a throwaway spike branch.

### 2026-11-07 · NOC-12 · RFC 0006 recommends one SQLite binding (rusqlite) on every target

- Decided by: coding agent (NOC-12), for the owner to confirm in the RFC's decision 3
- Why: rusqlite drives sqlite-wasm-rs in the browser, so the fold is one body of Rust code (principle 7); the spike shows it works with OPFS in a worker.
- Instead of: the official SQLite wasm build or wa-sqlite, both driven through JavaScript, which would fork the binding by platform.
