# 2026-11-12 · NOC-270 · S3 keys name their ids in lowercase hex, and the epoch item's key in `spaces` is the five bytes `epoch`

- Decided by: coding agent (NOC-270)
- Why: RFC 0007 names the keys `frames/<space id>/<change id>` and `uploads/<upload id>` but not how an id is written; lowercase hex is what the repository's logs and debug output use and is safe in any S3 key. A space id has 16 bytes, so a 5-byte key can never be a space's, and the reserved item needs no second key attribute.
- Instead of: base64url ids in the keys, or the epoch in a table of its own.
- Topics: relay, aws, rfc-0007
