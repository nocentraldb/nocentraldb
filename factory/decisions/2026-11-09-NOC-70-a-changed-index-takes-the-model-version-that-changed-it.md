# 2026-11-09 · NOC-70 · A changed index takes the model version that changed it

- Decided by: coding agent (NOC-70)
- Why: RFC 0005 gives each index a `since`, the version that added it, and accepts changing an index. When an index keeps its name but changes its fields or `equal`, the build sets its `since` to the new model version, so the lock says from which version on the index has its current form; an unchanged index keeps its `since`.
- Instead of: keeping the first `since` of the name, which would describe an index that no longer exists.
- Topics: model-lock, rfc-0005, indexes
