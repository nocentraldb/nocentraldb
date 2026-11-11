# 2026-11-11 · NOC-226 · The equivocation tests pin today's stability behaviour, the unsafe parts included

- Decided by: coding agent (NOC-226), within pm's scope (tests only; report a convergence break, do not fix it)
- Why: NOC-224 asks the owner how to close the hole an equivocating or lying admin opens in RFC 0008's stability. Until the owner decides, a test that asserted the wanted behaviour would fail with no fix allowed in this ticket, and a test that asserted nothing would let the behaviour drift unseen. So `membership_equivocation.rs` asserts what the core does now: an admin's fork below a counted sequence, or its new change that leaves the shred out, rejects a shred that was stable; a former admin's fork inside a senior demotion's cutoff does so only when its change id sorts before the change it duplicates. Each test says it changes on purpose with NOC-224. The convergence properties, which must hold whatever NOC-224 decides, assert agreement only: equal state, verdicts, sealed verdicts and stable changes on every copy that holds the same changes.
- Instead of: asserting the wanted behaviour and marking the tests ignored, which the gates forbid; leaving the behaviour untested.
- What this does not do: it changes nothing in `src/`, and it does not decide NOC-224.
- Topics: membership, rfc-0008, erasure, tests
