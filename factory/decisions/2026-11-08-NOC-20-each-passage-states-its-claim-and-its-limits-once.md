# 2026-11-08 · NOC-20 · Each passage states its claim and its limits once, and a test keeps it that way

- Decided by: Carsten (wording), coding agent (NOC-20) for the test
- Why: the owner's review of #45 asked that no passage take back an earlier sentence. The relay passages in RFC 0001, the FAQ, use case 2 and GENESIS.md now say in one go what the relay cannot read, what it sees and what it can do, and the erasure passages in RFC 0001, the FAQ and use case 1 claim unreadability only where the key is gone, naming the limits in the same statement. `scripts/rfc-0001.test.mjs` fails on "that is not all", "more than that", "however, it also" and "after all" in these five files, and on an unqualified "every device" or "everywhere" in the erasure passages, so a later edit cannot bring the pattern back unnoticed.
- Instead of: rewriting the passages without a test, which would leave the next edit free to append a correction again.
- Topics: docs, rfc-0001, genesis, review
