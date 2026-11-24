# 2026-11-24 · NOC-511 · In the merge queue, the changes check compares with the queue commit's first parent and reads trailers from the pull request's own commits

- Decided by: coding agent (NOC-511)
- Why: the queue's commit squashes the pull request onto main with the pull requests ahead of it, so `HEAD^1` holds everything that lands before it, as on the merge ref (NOC-505), whatever merge method the queue uses. The squash commit's message does not keep the trailers of the pull request's commits in its last paragraph, so the test-change check reads them from the pull request's head, fetched as `refs/pull/<number>/head`, with the number from the queue's branch name.
- Instead of: `merge_group.base_sha`, whose meaning for a pull request behind others in the queue GitHub does not pin down; or the trailers of the squash commit, which depend on the repository's squash-message setting.
- Topics: ci, release, tests
