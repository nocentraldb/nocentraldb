# 2026-11-09 · NOC-51 · An unknown argument parks before the shape is checked; keys 0 to 3 an operation does not define are malformed

- Decided by: coding agent (NOC-51)
- Why: RFC 0004 Amendment 1 does not say which reason wins when an operation has both an unknown "must understand" key (4 to 63) and a malformed shape. The decoder checks the number first (unknown operation), then for keys 4 to 63 (unknown argument), then the shape (malformed arguments): a later rule version may define the key together with a new shape and fold the operation after an upgrade, while a malformed operation stays parked on every version. A key from 0 to 3 that the operation does not define (`observed` on a set, `replaces` on a remove, key 0) is malformed, as the amendment makes key 3 on a delete; a text key is malformed too, since argument keys are integers. The order depends only on the bytes, so every member parks the same way.
- Instead of: malformed first, which would park for good an operation that an upgrade could read.
- Topics: rfc-0004, encoding, parking
