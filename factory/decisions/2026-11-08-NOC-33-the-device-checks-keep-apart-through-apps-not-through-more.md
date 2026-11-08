# 2026-11-08 · NOC-33 · The device checks keep apart through apps, not through more hosts

- Decided by: coding agent (NOC-33)
- Why: storage, Safari's seven-day counter and a `persist()` grant belong to the origin, so two checks on one host spoil each other. A Dock app on macOS and a home-screen app on iOS keep their own storage, so one host serves the check without `persist()` in Safari and the check with it in the app. The page reads on load, so the day-8 visit records the result before any tap counts as interaction.
- Instead of: a second host or port per check, which the owner would have to set up.
- Topics: browser-storage, manual-checks
