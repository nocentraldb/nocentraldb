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

### 2026-11-07 · NOC-10 · The wasm32 tests run with wasm-bindgen-test in Node

- Decided by: coding agent (NOC-10)
- Why: `wasm-bindgen-test` is the standard runner for `wasm32-unknown-unknown`, the browser target, and Node needs nothing the gate does not already have; CI runs it through `pnpm gates` without a workflow change. The runner (`wasm-bindgen-cli =0.2.130`, the `wasm-bindgen` that Cargo.lock holds) is built by `cargo install --locked` into `target/tools`, which the existing Cargo cache keeps; a cold build takes about a minute.
- Instead of: a headless Chromium run (needs a browser in CI for no extra coverage of the core today), a `wasm32-wasip1` target (it has `std::fs`, so it would hide exactly what the gate must catch), or downloading a prebuilt runner binary (a second pinning and checksum path).

### 2026-11-07 · NOC-10 · Test files route #[test] to wasm_bindgen_test on wasm32

- Decided by: coding agent (NOC-10)
- Why: wasm-bindgen-test skips plain `#[test]` and the runner still exits 0 ("no tests to run!"), so the gate would pass with no test run. Each test file imports `wasm_bindgen_test as test` under `cfg(target_arch = "wasm32")`, the tests stay unchanged natively, and `scripts/wasm-gate.test.mjs` fails when a file of ncdb-core has a `#[test]` without that import.
- Instead of: writing `#[wasm_bindgen_test]` next to every `#[test]`, which a new test can forget without any check noticing.

### 2026-11-07 · NOC-10 · The build detector uses std::os::unix, not std::fs

- Decided by: coding agent (NOC-10)
- Why: `std::fs` compiles for `wasm32-unknown-unknown` and fails only when called, so it cannot prove the build task fails; the fixture uses `std::os::unix`, which does not exist there. A call into `std::fs` at runtime is caught by the test-wasm task instead, as long as a test reaches it.
- Instead of: the ticket's example `std::fs`, which would have made the detector test pass the build and prove nothing.

### 2026-11-07 · NOC-14 · minicbor reads the CBOR tokens; the core checks the profile itself

- Decided by: coding agent (NOC-14)
- Why: RFC 0002's profile is stricter than any library's deterministic mode, so the core decodes into its own `Value`, rejects indefinite lengths, tags, other simple values, key types, unsorted and repeated keys and invalid UTF-8 as it reads, and then encodes again and compares, which catches every non-shortest integer, length and float and every other NaN. minicbor (`=2.3.1`, `alloc` only) supplies the tokens without serde and without `std`; floats are written by the core, because minicbor's half-precision support needs the `half` crate. The test vectors are written by `tests/vectors/generate.py` with Python's cbor2 and hashlib, independently of the crate; the gates check the committed vectors, they do not run the script.
- Instead of: ciborium with serde (its canonical output is not the profile, and the re-encode check would still be needed), or a hand-written tokenizer.

### 2026-11-07 · NOC-14 · Each red step compiles against a stub API

- Decided by: coding agent (NOC-14)
- Why: a Rust test that calls a function that does not exist fails to compile, which is the "missing import" failure `roles/coder.md` excludes. The first commit adds the types and signatures with `todo!()` bodies, so each red commit shows its tests failing at run time with "not yet implemented", or for the behaviour (the derived `Debug` of the redaction step).
- Instead of: red commits that fail to compile.

### 2026-11-07 · NOC-14 · Foldable means format 1, a known kind, and no unknown key from 1 to 63

- Decided by: coding agent (NOC-14)
- Why: RFC 0002 says a higher header or payload format is stored, not folded; this core knows only format 1, and no lower format exists, so any other number is not foldable. The padding (payload key 5) is kept as given and not checked for zeros, since "the reader ignores" it, and a header whose author member id is not 32 bytes is rejected, since RFC 0002 defines it as RFC 0003's SHA-256 member id. Whether op 0 carries no field is left to the fold, not the codec.
- Instead of: rejecting non-zero padding or accepting member ids of any length.

### 2026-11-07 · NOC-14 · Debug shows ids and ordering metadata, never content

- Decided by: coding agent (NOC-14)
- Why: GENESIS.md forbids content and keys in logs and errors. `Value` and `Key` show their kind and size; an operation shows only its unknown keys, since RFC 0002 counts object ids, collection and field names, ops and values as content; the sealed payload, padding and signature show only their length. Space, member and change ids, sequence numbers, key generations, formats, kinds, clocks and dependencies show, as hex where they are bytes: the relay or a member sees them without content. Errors name keys and byte offsets only.
- Instead of: derived `Debug`, which prints every byte, or hiding ids too, which leaves nothing to debug with.

### 2026-11-07 · NOC-14 · Random ids panic without a secure generator; the clock takes the wall clock as a function

- Decided by: coding agent (NOC-14)
- Why: an id from a weaker source could collide, so `ObjectId::random` and `SpaceId::random` panic when `getrandom` fails rather than fall back; on wasm32 getrandom's `wasm_js` feature uses `crypto.getRandomValues`. `Clock::now` takes the wall clock as a closure, so the core reads no time itself, the tests drive the counter overflow, and the waiting loop asks it again; a wall clock beyond 48 bits of milliseconds reads as the largest the clock holds.
- Instead of: a fallible id API every caller must handle for a failure the platform does not have, or a clock that reads `SystemTime` itself, which does not exist on wasm32-unknown-unknown.

### 2026-11-08 · NOC-14 · CBOR values nest at most 32 deep, counted from the frame

- Decided by: pm, pending the owner (NOC-24)
- Why: the decoder recursed without a limit, so a frame of nested one-item arrays overflowed the stack of a member or the relay, after a few hundred levels in a debug build and sooner on wasm32. `cbor::MAX_DEPTH` is 32; the frame is at depth 1, the header map at 2 and the payload map at 3, since a byte string that holds encoded CBOR does not reset the count. The depth is checked before a value is read, so 100,000 levels fail with `Reason::TooDeep`. The encoder still writes any depth; a value too deep for the decoder is refused on read.
- Instead of: no limit (a crash), or a limit per map that a header could multiply by nesting payload inside it.

### 2026-11-08 · NOC-14 · A full clock counter carries into the next millisecond

- Decided by: pm, pending the owner (NOC-25)
- Why: RFC 0002 has the writer wait for its wall clock when the counter is full. After a clock from the future was seen, that wait lasts as long as the other member's clock is ahead, and forever after `observe(Hlc(u64::MAX))`, which the first implementation did as a busy loop. `Clock::now` now takes one more than the highest clock seen, which carries into the next millisecond, never waits and never runs backwards, and returns `ClockExhausted` at `u64::MAX` rather than wrap. It takes the wall clock as a value, since it no longer asks again. Reverses part of "Random ids panic without a secure generator; the clock takes the wall clock as a function".
- Instead of: waiting for the wall clock, as RFC 0002 says.

### 2026-11-08 · NOC-14 · Key 0 is "must understand"

- Decided by: pm
- Why: RFC 0002 names keys 1 to 63 "must understand" and 64 up "may ignore", and leaves key 0 out. A key the RFC does not place cannot be safe to ignore, so a header, payload or operation with key 0 is kept and not folded.
- Instead of: treating key 0 as "may ignore", as the first implementation did.

### 2026-11-07 · NOC-22 · RFC 0002's amendment is a new section plus marked sentences, and keeps "refused by the relay" apart from "rejected"

- Decided by: coding agent (NOC-22)
- Why: the amendment needs one place for the owner's decisions D10 to D12, and the old sentences must not contradict it, so each changed sentence carries an *(Amendment 1)* mark and a dated changelog line sits at the top. "Rejected" is a log state that only the member's own checks set; the relay's refusal is recorded beside it, so the state stays a function of the changes a member holds (principle 2).
- Instead of: rewriting the sections silently, or letting RFC 0007's "marked as rejected" for a relay refusal set the log state.

### 2026-11-07 · NOC-7 · RFC 0003's tooling test checks the RFC's shape, not its design

- Decided by: coding agent (NOC-7)
- Why: a test can prove the file exists, links to RFC 0001, resolves its links and has a section per acceptance criterion; whether the cryptography is right is the owner's review of the PR.
- Instead of: no test at all, or a test that pins the RFC's recommendations, which would have to change with every owner decision.

### 2026-11-08 · NOC-7 · RFC 0003: every recommendation accepted

- Decided by: Carsten
- Why: the owner accepted all fifteen decisions and the six readings of RFC 0003: Ed25519 and X25519 member keys in the platform keystore, HPKE-wrapped space keys with generations, one person key per person per space, the account key on every device plus a recovery code, eager rotation, and the formats of decision 15.
- Instead of: the alternatives listed for each decision in `docs/rfc/0003-keys-and-cryptography.md`.

### 2026-11-07 · NOC-12 · The spike ran in Chromium only, and its code is kept in the PR, not on a branch

- Decided by: coding agent (NOC-12)
- Why: the cloud container has only Chromium; the RFC states Firefox and Safari as an open check for NOC-10 or the implementation ticket instead of claiming them. The code goes into the PR description because the agent may push only to the ticket's branch.
- Instead of: installing other browsers in the container, or pushing a throwaway spike branch.

### 2026-11-07 · NOC-12 · RFC 0006 recommends one SQLite binding (rusqlite) on every target

- Decided by: coding agent (NOC-12), for the owner to confirm in the RFC's decision 3
- Why: rusqlite drives sqlite-wasm-rs in the browser, so the fold is one body of Rust code (principle 7); the spike shows it works with OPFS in a worker.
- Instead of: the official SQLite wasm build or wa-sqlite, both driven through JavaScript, which would fork the binding by platform.

### 2026-11-08 · NOC-12 · RFC 0006: every recommendation accepted

- Decided by: Carsten
- Why: one database per member; the view's layout is not a stable format, only the log is; rusqlite everywhere, with opfs-sahpool in a dedicated worker in the browser; one tab owns the database; UTF-8 byte order in indexes; device disk encryption instead of an encrypted database for v1; synchronous = FULL; persistent storage requested by the SDK; secure_delete on.
- Instead of: the alternatives listed for each decision in `docs/rfc/0006-local-view.md`.

### 2026-11-08 · NOC-19 · The harness reports which contract property a case breaks, and shrinks within it

- Decided by: coding agent (NOC-19)
- Why: a broken fixture must be reported as the property it violates (C6), so the harness checks a case in a fixed order (C1 state bytes, C2 double application, C7 reads, C4 outcomes) and shrinks only towards cases that break the same property as the first failure. A divergence whose minimal case still needs a duplicate delivery is reported as C2, one that still needs a malformed operation as C5.
- Instead of: one pass/fail verdict, which cannot prove that each check fires on its own; or letting proptest shrink across properties, which reported a C1 divergence as a C2 double application on some seeds.

### 2026-11-08 · NOC-19 · Simulated members learn arbitrary earlier operations, not causally closed sets

- Decided by: coding agent (NOC-19)
- Why: RFC 0004 says no field type may rely on causal delivery, so before each operation a member learns a random subset of earlier operations, without what their authors had seen. C4 checks "replaced" against exactly that set.
- Instead of: learning an operation together with everything its author had seen, which would hide merge rules that assume causal delivery.

### 2026-11-08 · NOC-19 · C4 does not check what a malformed operation claims its author had seen

- Decided by: coding agent (NOC-19)
- Why: a malformed operation's body can claim to have seen anything, and the harness cannot tell a lie from a claim; C4 still requires every copy to give it the same outcome, and C1/C5 still require convergence.
- Instead of: holding malformed replacers to the simulated member's view, which would reject every type that, like RFC 0004's rules, trusts the dependencies a change carries.

### 2026-11-08 · NOC-19 · C3 is a committed SHA-256 over the final state bytes, with sha2

- Decided by: coding agent (NOC-19)
- Why: the same test runs natively and in wasm32 (NOC-10) and compares with one committed value, so neither run needs the other's output. `sha2 =0.11.1` is the audited crate and version PR #41 pins for change ids; the harness uses that same dependency, so it adds nothing to the shipped crate.
- Instead of: hashing with a hand-written function (GENESIS.md: never invent cryptography), or comparing the two runs' outputs in a separate script.

### 2026-11-08 · NOC-19 · A panic in apply fails the test instead of becoming a reported C5 violation

- Decided by: coding agent (NOC-19)
- Why: on wasm32 a panic aborts and cannot be caught, and behaviour must not fork by platform (principle 7); a panic still fails the gate on both targets. The C5 fixture breaks convergence on malformed operations instead of panicking.
- Instead of: catching panics natively with `catch_unwind`, which would report C5 on one target only.

### 2026-11-08 · NOC-16 · The field-type trait is `FieldType` in `ncdb_core::field`; the field is `&self`, the state apart

- Decided by: coding agent (NOC-16)
- Why: RFC 0004's "What a field type defines" names apply, state bytes, read and outcome; a value of the trait is the field as the schema declares it (a basic value's kind, a choice's options), so the same state reads differently under a newer schema without changing the state. `outcome` returns `None` for an operation the state never received. The module is `pub` only because the integration tests and the harness adapter in `tests/support/` must reach it; `ncdb-core` is not published and the names are working names, not the SDK's (RFC 0004, decision 7).
- Instead of: a trait on the state type alone, like the harness's `Mergeable`, which cannot carry the schema; or `pub(crate)` with the tests inside `src/`, which the ticket's adapter in `tests/support/` cannot reach.

### 2026-11-08 · NOC-16 · A basic value's state keeps every write it received

- Decided by: coding agent (NOC-16)
- Why: RFC 0004's R and L are computed from that set, so the state is a function of the set by construction; `outcome` can name the replacing write, and a duplicate is recognised by its id. The replaced ids R already grow with every overwrite (RFC 0004, limit 5); keeping the replaced writes' values too grows the state further until compaction.
- Instead of: keeping only L and R, which loses who replaced a write and needs the replacing operation id stored beside R anyway.

### 2026-11-08 · NOC-16 · How basic values answer what RFC 0004 leaves open

- Decided by: coding agent (NOC-16)
- Why: (1) a live write whose value equals the winner's is left out of `conflicts`, as the RFC says, and its outcome is visible, not lost, since its value is what the app reads; (2) `Replaced` names the replacing write with the lowest operation id, one deterministic choice among several; (3) the empty value is CBOR `null` and every kind allows it; (4) a value of the wrong kind that arrives (only from a malformed or newer writer) is kept and read with `known: false`, like an unknown choice option, so nothing is dropped; (5) a self-replacing malformed write is replaced by itself, as R ∪ L in the RFC computes it.
- Instead of: counting an equal-valued conflict as lost although the app cannot see it, or dropping values of the wrong kind on arrival.

### 2026-11-08 · NOC-16 · The state bytes are the local view's canonical CBOR, not a wire format; the wire encoding of set is not defined here

- Decided by: coding agent (NOC-16), flagged to pm
- Why: C1 and C3 need canonical state bytes, which RFC 0002's profile gives (`[[id, author, stamp, value, [replaced id…]]…]`); RFC 0006 makes the view's layout no stable format. The operation number of set and how `replaces` travels in an RFC 0002 operation are a wire format, which GENESIS.md reserves for the owner, and no RFC fixes them yet, so this ticket builds no encoder or decoder for them.
- Instead of: choosing an operation number and a `replaces` key in this PR.

### 2026-11-08 · NOC-16 · A harness generator for C3 draws no `usize`

- Decided by: coding agent (NOC-16)
- Why: proptest draws a `usize` differently on 32-bit wasm32 than on 64-bit native targets, so the two runs generated different cases and the C3 digest differed although the type was right. The basic values adapter draws `u8`; NOC-17's adapter must avoid `usize` too.
- Instead of: committing two digests, one per target, which would hide a real platform difference.

### 2026-11-08 · NOC-17 · How sets answer what RFC 0004 leaves open

- Decided by: coding agent (NOC-17)
- Why: (1) a set's state keeps every operation it received, as basic values do, so T, the live additions and the lost removals are computed from the set of operations; (2) an addition is `Replaced` by the removal with the lowest operation id that observed it, a removal by the addition with the lowest operation id that names it; a removal no addition names is lost while its element is in the set and visible (applied) when it is not; (3) a re-add names the removals of the element its author's copy holds that no addition has replaced yet; (4) a name of the wrong kind (an addition naming an addition, a removal naming a removal), which only a malformed operation writes, has no effect; (5) a set refuses the empty value as an element; (6) the state bytes are `[[id, author, stamp, element, 0 | 1, [named id…]]…]` in operation id order, the local view's canonical CBOR, not a wire format (NOC-37).
- Instead of: keeping only T and the live additions, which loses who replaced what; or letting a re-add name every removal it ever saw, which grows the state with ids already replaced.

### 2026-11-08 · NOC-17 · Links reuse basic values and sets; resolution is a read against the copy's objects

- Decided by: coding agent (NOC-17)
- Why: RFC 0004 gives links no merge rules of their own, so `Link` merges by the code of basic values and `Links` by the code of sets, over object ids (16 bytes) instead of a `Kind`. The crate-internal `basic::Values` says which values a field allows; `Kind` gets no new variant, because adding one to that public enum is a breaking change by cargo-semver-checks (CI, PR #53). Both implement `FieldType`, and the harness runs them as themselves. The status is not state, so it is not `FieldType::read`: `resolve` takes the copy's objects (`link::Objects`). The writing side takes the target with its space and refuses another space (`OtherSpace`); a removal takes any object id, so a stray link can be removed.
- Instead of: link field types with their own state, or a status stored in the state, which RFC 0004 rules out.

### 2026-11-08 · NOC-17 · What links resolve against: an object's creates and deletion, both basic values

- Decided by: coding agent (NOC-17), flagged to pm
- Why: the ticket asks for a test of two creates of one object id, and RFC 0004 defines what they mean: one object, its collection a basic value won by the create last in stamp order, creates replacing nothing; whether it is deleted is a boolean basic value (owner decision 4). `object::Object` holds exactly that, so the rule lives in the core and converges by the argument for basic values. The statuses are checked in this order: no create arrived (a deletion alone included) or a value that is no object id reads **missing**; then **deleted**, whatever the collection; then **wrong collection**; else **ok**. Object deletion as an operation, and folding creates out of RFC 0002 changes, belong to later tickets.
- Instead of: a test-only object model, which would prove nothing about the core; or reading a deleted target in another collection as wrong collection, which would hide that it is gone.
