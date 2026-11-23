# 2026-11-21 · NOC-430 · A rich-text Yjs client id derives from its member and a nonce, is checked on fold, and is reused through a device-local lease

- Decided by: Carsten, 2026-11-21, on RFC 0013's D1, through pm
- Why: two updates that give one Yjs item id different content make copies diverge, so a client id must name one writer that no other member can claim; a lease keeps the state vector to one id per device and parallel editor.
- Instead of: random client ids as Yjs draws them, which any member can reuse; one client id per change, which grows the state vector by one client per change.
- Topics: rich-text, rfc-0013, yjs, convergence
