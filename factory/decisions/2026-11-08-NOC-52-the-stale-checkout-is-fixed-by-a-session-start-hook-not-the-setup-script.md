# 2026-11-08 · NOC-52 · The stale checkout is fixed by a session start hook, not the setup script

- Decided by: coding agent (NOC-52)
- Why: a new cloud session restores the container from a snapshot taken when the environment was built (2026-11-07, at 90dcf81); the setup script does not run again, as `/root/.aws/config`, which it rewrites every run, still carries that date. The session start then moves HEAD to the fetched main with the index and working tree untouched (the reflog entry without a message) and checks the ticket branch out from the same commit, which touches no file. Only a hook that runs at every session start can repair that, and `.claude/settings.json` is the repository's.
- Instead of: a change to the setup script, which runs only when the snapshot is built and never sees the moved HEAD.
- Topics: factory, cloud, git
