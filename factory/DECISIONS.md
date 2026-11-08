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

### 2026-11-07 · NOC-8 · RFC 0004 needs five properties from RFC 0002's clock, not a specific clock

- Decided by: coding agent (NOC-8)
- Why: RFC 0002 runs in parallel; RFC 0004 lists what it needs (unique change ids, a total stamp order computed from the change alone, causality, wall-clock component wanted, one canonical encoding per value) so the two RFCs can be reconciled before merge.
- Instead of: assuming a hybrid logical clock and its encoding in RFC 0004.

### 2026-11-07 · NOC-8 · "Late" in principle 3 becomes "lost to a change its author had not seen"

- Decided by: coding agent (NOC-8), flagged to pm and the owner in RFC 0004
- Why: arrival time differs per member, so recording it in the converged state breaks principle 2, which ranks higher.
- Instead of: storing an "arrived late" mark in the field state.

### 2026-11-07 · NOC-8 · RFC 0004 names RFC 0002's clock as its stamp

- Decided by: coding agent (NOC-8), after RFC 0002 (PR #28) met O1 to O5
- Why: RFC 0002 fixed the stamp as a 64-bit hybrid logical clock and carries the author member id in the header; RFC 0004 now names both and keeps O1 to O5 as its reasoning. Updates the entry "RFC 0004 needs five properties from RFC 0002's clock, not a specific clock".
- Instead of: keeping the stamp abstract after RFC 0002 had fixed it.

### 2026-11-07 · NOC-9 · The convergence harness generates and shrinks with proptest

- Decided by: coding agent (NOC-9)
- Why: proptest shrinks through arbitrary strategy compositions, so a divergence shrinks without hand-written shrinkers, and its runner takes an explicit `TestRng`; without default features (`alloc`, `no_std`) it pulls no threads, no fork and no OS randomness (no getrandom), so the harness stays usable on wasm32 (NOC-10). Pinned to `=1.12.0` as a dev-dependency only.
- Instead of: quickcheck, whose shrinking does not compose through mapped generators, or a hand-rolled generator and shrinker.

### 2026-11-07 · NOC-9 · The harness lives in `crates/ncdb-core/tests/support/`, not in `src/`

- Decided by: coding agent (NOC-9)
- Why: the ticket keeps the `Mergeable` trait internal to the crate's test support; integration tests reach it with `mod support;`, and nothing of it enters the public API or cargo-semver-checks.
- Instead of: a `test-support` cargo feature that would expose the trait from `src/`.

### 2026-11-07 · NOC-9 · The harness seeds a fixed ChaCha generator and finishes shrinking itself

- Decided by: coding agent (NOC-9)
- Why: without proptest's `std` feature, `Config::rng_seed` is ignored, so the harness builds a ChaCha `TestRng` from `Config::seed` (default 0) and every run of the gate is the same. Arrival keys travel with each change so that shrinking a change keeps the others' order, and a final pass removes single changes and duplicate deliveries that proptest's vector shrinker leaves behind; minimality is tested for seeds 0 to 63.
- Instead of: OS-random seeds per run (not deterministic, not on wasm32), or accepting counterexamples that are not minimal.

### 2026-11-07 · NOC-8 · RFC 0004: every recommendation accepted

- Decided by: Carsten
- Why: the owner merged RFC 0004, which accepts its recommendations: last writer wins on read with losing writes kept as conflicts, ties broken by author member id then operation id, add wins for sets, deletion as a converged boolean until its own RFC, links to absent targets kept as missing, no field type relies on causal delivery, and working names until the SDK's API review.
- Instead of: the alternatives listed for each decision in `docs/rfc/0004-merge-rules-basic-types.md`.

### 2026-11-07 · no ticket · "Late" in principle 3 means lost to a change its author had not seen

- Decided by: Carsten
- Why: the time a change arrives differs per member, so storing it in the shared state would break principle 2; what every member can identify the same way is that a change lost to a change its author had not seen (RFC 0002, RFC 0004). GENESIS.md principle 3 and its no-go now say so.
- Instead of: "arrives late", which no converged state can record.
