# 2026-11-08 · NOC-13 · A relay accepts a new space only from an account on its operator's allowlist, not from anyone who reaches it

- Decided by: Carsten (RFC 0007 decision R6)
- Why: a relay must not become free storage for strangers and a bill for its operator. The relay checks it on the genesis, whose member certificate names the account.
- Instead of: anyone who can reach the relay; anyone with an invitation code from the operator.
- Topics: relay, rfc-0007, accounts, cost

The rule stands in `docs/rfc/0007-relay.md`, "What the relay checks", "Who may create a space". Its one exception, one account space for an account not on the allowlist, came later ([NOC-283](2026-11-12-NOC-283-a-guest-accounts-one-space-is-an-effect-of-its-genesis-append.md)).
