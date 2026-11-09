# 2026-11-09 · NOC-70 · Removing a computed field or a view name is accepted

- Decided by: coding agent (NOC-70), for pm's review
- Why: RFC 0005 lists adding a computed field or a view as accepted and says nothing about removing one. Like an index, a computed field or a view is derived locally and never travels in a change, so removing its name loses no data and no old client notices; the additive check accepts it, as it accepts removing an index.
- Instead of: refusing it, which would keep a dead name in every lock forever without protecting any data.
- Topics: model-lock, rfc-0005, additive-change
