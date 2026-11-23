# 2026-11-22 · NOC-457 · A rich-text update's clocks stay below 2^30 and its client ids below 2^53, or it parks

- Decided by: coding agent (NOC-457)
- Why: the core's format check parks what no honest writer produces, so that it and Yrs never read one update two ways: a client id beyond what a JavaScript number holds, a clock or a range that ends at 2^30 or beyond (a billion characters from one writer of one field), any other number beyond 32 bits, a count larger than the bytes left, text that is not UTF-8, values nested deeper than 64, an empty struct, a client listed twice, and a struct that names its own writer's id at or after its own clock.
- Instead of: no limits of the core's own, which leaves the reading of every number to the library that applies the update.
- Topics: rich-text, rfc-0013, yjs, robustness
