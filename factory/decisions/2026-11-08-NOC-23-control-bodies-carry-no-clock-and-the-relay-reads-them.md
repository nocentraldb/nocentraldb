# 2026-11-08 · NOC-23 · Control bodies hold an entry type, control dependencies and fields from key 8, no clock; the relay reads them

- Decided by: Carsten (RFC 0008 decisions M11, M18 and M22)
- Why: control bodies and their dependencies are readable so the relay runs the same rules and can stop a removed member filling the log; the price is that the relay sees roles, cutoffs, the account behind each member across spaces, the sub-agent tree, the erasure approvers and the generation graph. Key 2 stays free for a clock and is absent, because a clock would show the relay when admins act, which RFC 0002's D6 keeps from it. One membership entry with an action keeps the rules on grants and revocations in one place, and field numbers from 8 up leave RFC 0002's payload keys free for shared meanings; the layout is wire format and cannot change once shipped.
- Instead of: encrypted control bodies, with only members applying the rules and the relay checking signatures; the author's hybrid logical clock in control bodies; one entry type per action (add, set role, remove).
- Topics: rfc-0008, membership, relay, wire-format

The bodies are in `docs/rfc/0008-membership.md`, "The control bodies"; what the relay learns and checks is in "How the relay applies the rules".
