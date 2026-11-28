# 2026-11-27 · NOC-576 · Rich-text content is compared for the same id twice with every map in key order

- Decided by: Carsten, 2026-11-27, NOC-576, through pm
- Why: the digest of each run of ids an update adds hashed the content as its writer wrote it, so a writer using Yrs that sent the same items again, such as a member restored from a backup, could write a map in another key order, and its write parked as giving the ids other content. The core now takes the digests of the update with every map in key order, as `rich_text::update::canonical` writes it (NOC-540), so the same content gives the same digest whatever order its writer used. No release folds rich text yet, so no stored digest needs migrating.
- Instead of: hashing the content as written, so that a re-send with a map in another order parks.
- Topics: rich-text, rfc-0013, yjs, convergence
