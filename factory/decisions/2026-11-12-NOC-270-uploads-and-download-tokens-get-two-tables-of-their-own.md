# 2026-11-12 · NOC-270 · The AWS backend keeps uploads and download tokens in two tables of its own, `uploads` and `tokens`

- Decided by: coding agent (NOC-270)
- Why: RFC 0007's table of tables has no row for the uploads and download tokens the backend interface stores; a push takes an upload by its id, a URL finds it by its token, so `uploads` has the upload id as key and `tokens` the token, each read consistently, where one table with an index on the other key would read the index eventually consistent and could miss an upload just filled.
- Instead of: one table keyed by token with a global secondary index on the upload id, or the items in `spaces` under reserved keys.
- Topics: relay, aws, rfc-0007, uploads
