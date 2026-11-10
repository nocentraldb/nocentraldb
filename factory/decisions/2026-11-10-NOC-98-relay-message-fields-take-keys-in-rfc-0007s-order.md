# 2026-11-10 · NOC-98 · Relay message fields take integer keys in RFC 0007's order, after the type and the request id

- Decided by: Carsten, 2026-11-10, on NOC-98
- Why: RFC 0007's error map is the rule for every message: the type as text at key 1, the request id at key 2 or absent, then each field at keys 3, 4, … in the order RFC 0007 lists it; a nested map numbers its fields from 1 in RFC order; types and error codes stay text. Compact CBOR that a reader derives from the RFC. NOC-106 writes the rule and the full table into RFC 0007 ("The keys of each message", decision R12), taking NOC-89's table and the same rule for the messages NOC-89 does not build.
- Instead of: text keys for every field, larger in every message and unlike the error map; or numbers picked by hand with gaps, more to review for room CBOR maps already give.
- Topics: rfc-0007, relay, wire
