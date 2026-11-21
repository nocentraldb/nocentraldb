# 2026-11-21 · NOC-430 · A rejected rich-text change is un-folded by keeping its items as invisible placeholders and undoing its deletions

- Decided by: Carsten, 2026-11-21, on RFC 0013's D2, through pm
- Why: other members' later edits are placed next to the rejected change's characters; leaving its update out would leave them waiting for good, so an honest member's text would vanish. With placeholders the document reads as if the rejected change had never been written, and later edits stay. Text typed inside a block the rejected change added is `lost` and readable, and an attribute it set ends unset.
- Instead of: recomputing the document without the rejected update, as other field types do.
- Topics: rich-text, rfc-0013, un-folding, convergence
