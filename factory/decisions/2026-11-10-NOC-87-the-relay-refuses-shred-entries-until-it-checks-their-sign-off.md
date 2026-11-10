# 2026-11-10 · NOC-87 · The relay refuses shred entries until it checks their sign-off

- Decided by: coding agent (NOC-87)
- Why: RFC 0007 has the relay check that a shred entry is signed off by the account key of a second account. The core does not read a shred body yet (`Entry::Other`, NOC-67), so the relay cannot make that check, and accepting a shred on its author's right alone would let one member erase a person's data. So the relay answers a shred entry `unsupported_suite`: the member keeps it, and pushes it again to an upgraded relay. Person-key records (`put-person-keys`), whose check needs the person-key entry's body, are not accepted by the core either; the backends store them already.
- Instead of: appending shred entries on the right `resolve` checks, without the second account's sign-off.
- What this does not do: until the relay reads shred bodies, a shred reaches other members only directly, not through the relay.
- Topics: relay, rfc-0007, rfc-0008, shred, safe-subset
