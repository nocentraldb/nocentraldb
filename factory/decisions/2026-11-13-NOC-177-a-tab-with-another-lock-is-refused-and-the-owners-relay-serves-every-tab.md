# 2026-11-13 · NOC-177 · A tab with another lock than the owner's is refused, and the owner's relay serves every tab

- Decided by: pm, NOC-177, on the coding agent's proposal
- Why: two tabs of one origin can run two versions of an app. Only the owner's worker checks its lock on the device, so a tab whose lock differs from the owner's would read and write under rules it did not open with; the owner refuses its open with `modelRefused`, and once that tab owns the database, it opens with its own lock through the usual check. One worker keeps one relay connection, so the owner's relay serves every tab, and each tab's `db.sync.current().relay` names its own option. A cursor the old owner's worker kept fails after a handover with `cursorOfAnotherModel`, as one of a closed database does.
- Instead of: letting a tab with another lock use the owner's database, which skips RFC 0005's check on the device for it; or a connection per relay, which one worker per database does not have.
- Topics: browser, sdk-ts, tabs
