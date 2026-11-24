# 2026-11-23 · NOC-505 · The changes check compares a pull request with its merge ref's first parent

- Decided by: coding agent (NOC-505)
- Why: the checkout is the merge ref, so only `HEAD^1` holds everything main gained since the PR's last push; the baseline step fails unless `HEAD^2` is the PR's head, so `HEAD^1` is always that main.
- Instead of: the merge base of the event's base and head (NOC-503), which counted a breaking change main gained after the push against the PR.
- Topics: ci, release
