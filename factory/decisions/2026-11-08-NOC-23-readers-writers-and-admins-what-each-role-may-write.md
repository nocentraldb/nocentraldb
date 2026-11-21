# 2026-11-08 · NOC-23 · Readers, writers and admins: what each role may write, an agent's default role, a space without an admin

- Decided by: Carsten (RFC 0008 decisions M1, M13 and M14)
- Why: readers cover adapters, dashboards and auditors, which should not write; more roles can come later. Most agents in RFC 0001's examples write, so the SDK adds an agent as a writer unless the app asks for another role, and its notice names the role. A space that loses its last admin through concurrent removals keeps working for content, but its membership and key are frozen, because nobody should become admin without being chosen.
- Instead of: writer and admin only, or a set of capabilities per member (a check per capability on every member and the relay); an agent added as a reader unless the app asks for more (every writing agent a two-step setup), or no default role (longer calls for the common case); the most senior writer becoming admin automatically.
- Topics: rfc-0008, membership, roles

The table of rights per role, and what the roles do not do, are in `docs/rfc/0008-membership.md`, "Roles and rights".
