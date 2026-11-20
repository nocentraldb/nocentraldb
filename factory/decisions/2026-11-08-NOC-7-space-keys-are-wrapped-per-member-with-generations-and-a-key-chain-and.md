# 2026-11-08 · NOC-7 · Space keys are wrapped per member, with generations and a key chain, and rotate eagerly at every removal

- Decided by: Carsten (RFC 0003 decisions 1, 5 and 6, and reading 3)
- Why: of the group key protocols compared, only a per-member wrapped space key gives a new member the whole history through the key chain, which NoCentralDB promises; it needs no order of membership changes, so island mode keeps working, and costs the relay only opaque envelopes. The price is no forward secrecy within a generation and one envelope per remaining member at a removal, acceptable for small spaces and rare removals. The key rotates as part of every removal, because a removed member must not read anything written after it, even if nobody writes for a while; a member that has not yet seen the removal may still write under the old generation, and use case 3 says so.
- Instead of: MLS or sender keys (no history for new members; MLS needs one total order of commits); a new member reading only from the generation it joined (may come later as a per-space option); rotating lazily on the next write after a removal.
- Topics: rfc-0003, crypto, keys, rotation, membership

The rules are in `docs/rfc/0003-keys-and-cryptography.md`, "The space key", "Sharing a space key with a new member", "Removing a member and rotating the space key" and "Group key distribution: options compared".
