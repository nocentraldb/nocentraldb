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

### 2026-11-07 · NOC-6 · RFC 0002 names its conflicts with GENESIS.md and RFC 0001 instead of resolving them

- Decided by: coding agent (NOC-6)
- Why: parked changes (D9) bend principle 2, and the relay sees more metadata than RFC 0001 lists; both touch what GENESIS.md reserves for the owner, so the RFC proposes a reading and leaves the ruling to the owner.
- Instead of: editing RFC 0001, the FAQ or GENESIS.md in this PR, which is outside the ticket's scope.

### 2026-11-07 · NOC-6 · The docs-only RFC is held by a test in the repository tooling

- Decided by: coding agent (NOC-6)
- Why: `scripts/rfc.test.mjs` runs in `repo:test` and checks that RFC 0002 exists and links back to RFC 0001, so the gate fails if either is lost.
- Instead of: no test, which leaves the gate unchanged for a docs-only PR.

### 2026-11-07 · NOC-6 · RFC 0002: every recommendation accepted

- Decided by: Carsten
- Why: the owner accepted all of RFC 0002's proposals. That covers D1 to D9: HLC plus dependencies, SHA-256 change ids, 128 random bits for object ids, deterministic CBOR, field names in operations, the clock inside the payload, the log in the view's SQLite database, padding to 256 bytes, and parked changes. It also covers the reading of principle 2 for parked changes, and the proposal that RFC 0001 and the FAQ link to the RFC's list of what the relay sees.
- Instead of: the alternatives listed for each decision in `docs/rfc/0002-change-and-log-format.md`.

### 2026-11-07 · no ticket · A PR that waits only for the owner holds no lane

- Decided by: Carsten
- Why: four reviewed PRs waited for the owner and blocked all four lanes, so no ready ticket could start; the lanes now count only work in progress, and a waiting session stays open to answer review.
- Instead of: a lane held until merge, which keeps the owner's queue short but stops the factory while the owner reviews.

### 2026-11-07 · NOC-22 · RFC 0002's two gaps become an errata ticket, not fixes in the later RFCs

- Decided by: pm
- Why: RFC 0002 (merged) says a change is atomic and also parks single operations, and has no log state for a change that failed a check; RFC 0003, 0005, 0006 and 0007 each lean on one reading. An amendment to RFC 0002 settles it once, and the owner signs it off as a format change.
- Instead of: letting each later RFC define its own reading, which would leave four RFCs to reconcile later.

### 2026-11-07 · NOC-14 · The change and log implementation stacks on NOC-10 and NOC-9

- Decided by: pm
- Why: all three change `crates/ncdb-core`, and new test files need NOC-10's wasm32 test routing; stacking avoids merge conflicts in Cargo.toml and Cargo.lock while #29 and #34 wait for the owner.
- Instead of: branching from main and resolving conflicts after the owner merges #29 and #34.
