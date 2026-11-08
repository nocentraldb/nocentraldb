# 2026-11-08 · NOC-17 · What links resolve against: an object's creates and deletion, both basic values

- Decided by: coding agent (NOC-17), flagged to pm
- Why: the ticket asks for a test of two creates of one object id, and RFC 0004 defines what they mean: one object, its collection a basic value won by the create last in stamp order, creates replacing nothing; whether it is deleted is a boolean basic value (owner decision 4). `object::Object` holds exactly that, so the rule lives in the core and converges by the argument for basic values. The statuses are checked in this order: no create arrived (a deletion alone included) or a value that is no object id reads **missing**; then **deleted**, whatever the collection; then **wrong collection**; else **ok**. Object deletion as an operation, and folding creates out of RFC 0002 changes, belong to later tickets.
- Instead of: a test-only object model, which would prove nothing about the core; or reading a deleted target in another collection as wrong collection, which would hide that it is gone.
- Topics: field-types, links, objects, rfc-0004
