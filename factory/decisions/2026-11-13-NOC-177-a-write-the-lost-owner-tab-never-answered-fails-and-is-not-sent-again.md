# 2026-11-13 · NOC-177 · A write the lost owner tab never answered fails with storageFailed and is not sent again

- Decided by: pm, NOC-177, on the coding agent's proposal
- Why: when the tab that owns a browser database goes away, a call another tab sent it may or may not have run, and no tab can know which. A read is safe to send again, so it goes to the new owner. A write (`write`, `spaces.create`, `sync.pause`, `sync.resume`) sent again could be applied twice, so it fails with `storageFailed` and a message that says it may or may not have run; the app reads and decides. A unit test holds a write in the old owner and shows the new owner never runs it, and a Chromium test closes the owner's tab during a write and finds the write at most once.
- Instead of: sending every call again, which may apply a write twice; or a new error code such as `ownerClosed`, a public name for a case `storageFailed` with its message covers.
- Topics: browser, sdk-ts, tabs
