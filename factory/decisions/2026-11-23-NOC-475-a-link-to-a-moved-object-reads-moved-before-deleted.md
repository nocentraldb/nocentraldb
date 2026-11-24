# 2026-11-23 · NOC-475 · A link to a moved object reads `moved` before `deleted`, and a reference follows traces to `ok`, naming the space it found the object in

- Decided by: coding agent, NOC-475
- Why: a move and a delete are independent values, and a delete written in the old space after the move is a later edit; the object lives on in the new space, so a deleted trace still reads `moved` with that space. A reference's read carries `found`, the space whose copy holds the target, so an app can write the reference again to name the new space (RFC 0016, "References to the moved object").
- Instead of: reading `deleted` for a trace that was also deleted, which would hide where the object went.
- Topics: rfc-0016, move, links, references, resolution
