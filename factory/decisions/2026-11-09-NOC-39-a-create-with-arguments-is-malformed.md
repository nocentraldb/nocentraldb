# 2026-11-09 · NOC-39 · A create that carries key 5 parks as malformed arguments

- Decided by: coding agent (NOC-39)
- Why: RFC 0004 Amendment 1 says a create (operation 0) is written without key 5 ("keys 3 and 5 are absent") but does not say what a reader does with a create that carries one. Parking it as "malformed arguments" keeps exactly one encoding per operation, as the amendment's lists of ids do, and depends only on the bytes, so every member parks the same way. A later rule that gives create arguments would take a new operation number, as the amendment requires for every new operation.
- Instead of: ignoring key 5 on a create, which would give one create two encodings and let a later version change what an old client already folded.
- Topics: rfc-0004, encoding, parking
