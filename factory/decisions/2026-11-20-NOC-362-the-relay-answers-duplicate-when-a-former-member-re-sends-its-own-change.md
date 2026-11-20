# 2026-11-20 · NOC-362 · The relay answers duplicate when a former member re-sends its own change

- Decided by: Carsten, 2026-11-20, on NOC-362 ("Ja zu den Empfehlungen"); the relay has done so since NOC-276
- Why: a member's leave can be appended while its answer is lost. Answering `duplicate` to the change's own author before the membership check lets it learn the position its first push would have told it, so the leave is confirmed. RFC 0007 already said so; RFC 0002, which requires the uploader to be a current member, now names the exception.
- Instead of: keeping RFC 0002's rule without the exception, which the relay does not follow.
- Topics: relay, wire-format, rfc-0002, rfc-0007
