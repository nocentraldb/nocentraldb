# 2026-11-08 · NOC-16 · A basic value's state keeps every write it received

- Decided by: coding agent (NOC-16)
- Why: RFC 0004's R and L are computed from that set, so the state is a function of the set by construction; `outcome` can name the replacing write, and a duplicate is recognised by its id. The replaced ids R already grow with every overwrite (RFC 0004, limit 5); keeping the replaced writes' values too grows the state further until compaction.
- Instead of: keeping only L and R, which loses who replaced a write and needs the replacing operation id stored beside R anyway.
- Topics: field-types, rfc-0004, convergence
