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
