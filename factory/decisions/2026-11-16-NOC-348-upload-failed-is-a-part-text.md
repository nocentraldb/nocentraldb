# 2026-11-16 · NOC-348 · `upload_failed` lives in texts.json as kind `part`

- Decided by: coding agent, NOC-348
- Why: pm asked for the code's text and when it shows in texts.json. No kind is made for a sync-status code: `notice` and `error` need a NoticeCode or ErrorCode, which `upload_failed` is not, and `ts`, `cli` and `core` belong to other crates. A `part` has no code, generates a constant in the SDK's module, and the gate requires the code to use it, so the session reads the code from `texts::part::UPLOAD_FAILED` and the owner reviews it with the other texts.
- Instead of: a new kind for sync-status codes, which would change the generator and the gate for one entry; keeping the code only as a literal in the session, outside texts.json.
- Topics: texts, sync, sdk
