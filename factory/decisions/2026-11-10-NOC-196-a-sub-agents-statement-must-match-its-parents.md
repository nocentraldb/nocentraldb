# 2026-11-10 · NOC-196 · A sub-agent is added only when its key 6 matches its parent's, by recovery key

- Decided by: coding agent (NOC-196), inside the core, with pm (review of #178)
- Why: RFC 0003 says a sub-agent's certificate carries the statement of its parent's account, and RFC 0008 checks a sub-agent's certificate against its parent's. Key 6 is outside key 5's signature, so whoever writes the add can attach or strip it; the fold therefore accepts a sub-agent's add only when both certificates carry a statement under the same recovery key, or neither carries one. Comparing the recovery key, not the whole statement, lets a parent's account move to a newer account key number without its sub-agents' certificates becoming invalid; the account keys are already compared. The erasure sign-off still compares account keys: which recovery key the fold may trust is the owner's question NOC-201.
- Instead of: checking only the sub-agent's own statement, which lets it claim another account's recovery key; requiring the whole statement to be equal, which ties a sub-agent to one statement number of its parent.
- Topics: membership, crypto, rfc-0003, rfc-0008
