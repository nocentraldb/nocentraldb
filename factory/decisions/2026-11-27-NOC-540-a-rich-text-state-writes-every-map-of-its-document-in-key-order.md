# 2026-11-27 · NOC-540 · A rich-text state writes every map of its document in key order

- Decided by: coding agent (NOC-540)
- Why: Yrs keeps a value's maps (a mark's attributes, a node attribute that holds an object) in hash maps and encodes them in each hash map's order, so two copies holding the same document wrote different state bytes, which breaks C1. After Yrs encodes the state, the core walks the update with its own reader and writes each map again: in a value of lib0's encoding its entries sorted by key, byte by byte; in the JSON a mark's or embed's value is written as, objects with sorted keys and every other value as Yrs writes it. Nothing else changes, and the bytes encode the same document, so a binding loads them as before.
- Instead of: a fork or patch of Yrs that keeps maps in order, which the core would then have to carry; comparing documents rather than bytes, which gives up C1's byte equality.
- Topics: rich-text, rfc-0013, yjs, convergence
