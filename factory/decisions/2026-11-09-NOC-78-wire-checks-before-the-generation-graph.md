# 2026-11-09 · NOC-78 · Wire membership and envelopes into Checks before the generation graph

- Decided by: pm
- Why: After NOC-67 (#83) and NOC-68 (#80), two tickets could start: NOC-78, which wires `resolve` and key envelopes into `Checks` with D11's re-check, and NOC-71, the key chain and generation graph for re-grants. Both change the membership verdict on sealed changes, so running them at once would put two sessions on the same files. NOC-78 is on the path to the first end-to-end slice (NOC-66); NOC-71 only lifts a fail-safe rejection of re-grants. So NOC-78 starts now and NOC-71 waits for it.
- Instead of: starting both at once and resolving the overlap at merge time; or NOC-71 first, which delays the slice for a case the slice does not need.
- Topics: factory, lanes, membership
