# 2026-11-07 · NOC-8 · Last writer wins with losing writes kept as conflicts, add wins for sets, and absent link targets read missing

- Decided by: Carsten (RFC 0004 decisions 1, 2, 3 and 5)
- Why: basic values (and single links) read the write with the highest stamp, and the losing writes stay as conflicts until someone writes having seen them, which keeps RFC 0001's promise and principle 3; a tie on the stamp is broken by author member id, then operation id, so who wins depends on who wrote, not on a hash of the content. Sets let add win: no addition is lost, and a removal that lost stays readable. A link to a target the copy does not hold is kept and reads **missing** until the target arrives, so an app can show "waiting for data".
- Instead of: plain last writer wins with losing writes only in the log and archive, or every concurrent value exposed (multi-value register); a tie-break on operation id only; remove wins; hiding links to absent targets until they arrive.
- Topics: rfc-0004, merge-rule, convergence, linking

The rules are in `docs/rfc/0004-merge-rules-basic-types.md`, "Basic values", "Sets" and "Links".
