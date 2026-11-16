# 2026-11-16 · NOC-170 · Superseded change fragments stay until the beta notes are curated

- Decided by: pm (NOC-170)
- Why: `changes/shred-stability.md` (NOC-198) and `changes/relay-deletes-stable-shred-records.md` (NOC-113) describe behaviour that NOC-229 replaced before any release; a shred is now final when accepted (RFC 0008, "A shred is final when accepted"). NOC-170 left them in place and made them cite RFC 0008's earlier rule. Whoever curates the next beta's notes from the fragments merges each with its later reversal, such as `relay-keeps-stable-shred-records.md`, into what actually ships.
- Instead of: deleting the two fragments now, which loses the record of what was built and reverted before the notes are written; or rewriting them to describe the current behaviour, which the later fragments already do.
- Topics: release-notes, change-fragments
