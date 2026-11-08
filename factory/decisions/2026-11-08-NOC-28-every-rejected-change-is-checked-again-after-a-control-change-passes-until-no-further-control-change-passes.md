# 2026-11-08 · NOC-28 · Every rejected change is checked again after a control change passes, until no further control change passes

- Decided by: coding agent (NOC-28)
- Why: the end state must depend only on the set of changes. Checking all rejected changes again, bad signatures included, and repeating while a re-checked control change passes, is the simplest rule that reaches the same fixpoint in any order; the amendment allows skipping the ones that can never pass, an optimisation for later. This only ever moves changes from rejected to folded. RFC 0008 (membership) says a later revocation can make a folded change fail, and RFC 0006 then un-folds it; NOC-28's criteria do not ask for that, the doubles' control state only grows, and it is left to its own ticket.
- Instead of: skipping bad signatures and undecryptable payloads now, or one pass without repeating, which leaves a change rejected when its control change itself waited for another.
- Topics: rfc-0002, log, convergence
