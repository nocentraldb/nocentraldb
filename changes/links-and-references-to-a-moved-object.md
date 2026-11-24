---
type: breaking
tickets: [NOC-475]
components: [core]
---
## Problem
After a move, a link to the moved object could only read `deleted` or `missing`, a reference broke, and edits that reached the old space after the move were not identifiable (RFC 0016).

## Use
`link::Status::Moved(space)` is the status of a link to a moved object. A reference that names the old space follows the traces, up to 8 in a row, and `reference::Resolution::found` names the space whose copy holds the target. `ncdb_core::moves::later_edits(log, checks)` lists each moved object's later edits.

## Migration
`ncdb_core::link::Status` gains the variant `Moved`, and `ncdb_core::reference::Resolution` the field `found`; a `match` on the one and a struct literal of the other need them.
