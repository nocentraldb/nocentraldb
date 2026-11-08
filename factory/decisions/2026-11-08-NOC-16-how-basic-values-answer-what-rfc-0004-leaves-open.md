# 2026-11-08 · NOC-16 · How basic values answer what RFC 0004 leaves open

- Decided by: coding agent (NOC-16)
- Why: (1) a live write whose value equals the winner's is left out of `conflicts`, as the RFC says, and its outcome is visible, not lost, since its value is what the app reads; (2) `Replaced` names the replacing write with the lowest operation id, one deterministic choice among several; (3) the empty value is CBOR `null` and every kind allows it; (4) a value of the wrong kind that arrives (only from a malformed or newer writer) is kept and read with `known: false`, like an unknown choice option, so nothing is dropped; (5) a self-replacing malformed write is replaced by itself, as R ∪ L in the RFC computes it.
- Instead of: counting an equal-valued conflict as lost although the app cannot see it, or dropping values of the wrong kind on arrival.
- Topics: field-types, rfc-0004, convergence
