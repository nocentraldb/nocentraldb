# 2026-11-22 · NOC-447 · A move is operation 5 with the hidden field $movedFrom, called space.move (move_objects in Rust), with space.moves, the notice objectMoved, the error moved and the link status moved

- Decided by: Carsten, 2026-11-22, on RFC 0016's questions, through pm (NOC-449)
- Why: a move is a state of the object like delete, so it takes the first reserved number and an old core parks it; the names follow RFC 0009's conventions, and move is a keyword in Rust.
- Instead of: a set on a reserved field $moved, which a member without the special case would read as a write to an unknown field.
- Topics: move, format, public-api, rfc-0016
