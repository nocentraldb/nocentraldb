# 2026-11-10 · NOC-106 · RFC 0007's key table applies NOC-98's rule to the messages NOC-89's table leaves out

- Decided by: coding agent (NOC-106)
- Why: NOC-89's decision file lists the keys of the messages its server builds. The ticket asks for the full table, so the rule gives the rest the same way: the `keys` answer `{2: req, 3: space, 4: envelopes, 5: chain}`, the `person-keys` answer and `put-person-keys` with `4: records`, a record `{1: person-key id, 2: generation, 3: writer member id, 4: wrapped, 5: signature}`, the `upload` answer `{2: req, 3: id, 4: url}`, an entry's key 3 holding a frame's bytes or a download `{1: url, 2: size}`, `ok {2: req}` and the error map as RFC 0007 already had it. NOC-89's line "a request with a key its type does not define is malformed" is the server's behaviour, not part of the owner's rule, and stays out of the RFC.
- Instead of: listing only NOC-89's rows, which leaves the keys of five messages open in a wire format.
- Topics: rfc-0007, relay, wire
