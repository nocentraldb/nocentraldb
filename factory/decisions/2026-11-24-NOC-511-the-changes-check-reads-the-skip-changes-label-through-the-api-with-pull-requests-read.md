# 2026-11-24 · NOC-511 · The changes check reads a queued pull request's skip-changes label through the API, with pull-requests: read

- Decided by: coding agent (NOC-511)
- Why: the `merge_group` event names no pull request and carries no labels; without the label, a pull request that may skip its fragment would pass on the pull request and fail in the queue. `pull-requests: read` is read-only and the one permission the call needs.
- Instead of: leaving the fragment rule out in the queue, which the ticket keeps; or `issues: read`, which grants more than the labels of pull requests.
- Topics: ci, release
