# 2026-11-09 · NOC-40 · A payload key 64 of the wrong shape parks the change as "another model"

- Decided by: pm, in its review of #67
- Why: a key 64 that is not a map of a 16-byte model id and an unsigned model version does not show that the member's model wrote the change, so no lock vouches for its operations; parking it costs nothing that a correct writer would notice, and the state stays a function of the change's bytes.
- Instead of: treating a malformed key 64 as absent, which would fold a change by rules no lock is known to share.
- Topics: rfc-0002, rfc-0005, log, model
