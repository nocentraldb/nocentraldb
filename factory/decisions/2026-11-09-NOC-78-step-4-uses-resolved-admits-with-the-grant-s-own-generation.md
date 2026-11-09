# 2026-11-09 · NOC-78 · Step 4 uses `Resolved::admits`, with the grant's own generation until NOC-71

- Decided by: coding agent (NOC-78)
- Why: RFC 0008's membership verdict for a sealed change ("Sealed changes") reads only its header and the control state, so it lives on `Resolved`, where the relay can call it too. It admits a change when its author holds an accepted grant of a role that writes whose from generation is the change's key generation, and its author sequence is at most the cutoff of every accepted revocation of the right to write that the grant did not see; `resolve` records, per accepted grant, the revocations in its causal past. The generation graph is NOC-71, so "or descends from it" is not read yet, and a change under a later generation is "not a member" until then.
- Instead of: deciding step 4 inside `SealChecks`, which the relay could not reuse; or reading the effective role of a grant, which RFC 0008's cutoff rule already accounts for.
- Topics: rfc-0008, membership, relay
