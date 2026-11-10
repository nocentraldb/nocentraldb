# 2026-11-10 · NOC-142 · RFC 0007 draws the conditional person-key put in its text, without a new entry under Decisions

- Decided by: coding agent (NOC-142)
- Why: RFC 0007's "Decisions" lists the decisions the owner made (R1 to R12). The conditional put is a coding agent's decision (NOC-118), recorded in the decision log, so the RFC carries it in the text where it belongs (the protocol's person-key store, the backend sketch, the backend rules and their table, and the AWS `person_keys` table) and a dated History line names NOC-142 and #130. The owner's tick on the PR makes it part of the decided text.
- Instead of: a decision R13 under "Decisions", which would present an agent's call as one the owner decided before the owner did; or leaving the RFC behind the code and the conformance suite.
- Topics: relay, rfc-0007, person-keys
