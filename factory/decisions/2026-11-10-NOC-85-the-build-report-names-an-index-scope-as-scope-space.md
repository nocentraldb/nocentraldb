# 2026-11-10 · NOC-85 · The build report names an index's scope as "scope space" and lists a scope change as a changed index

- Decided by: coding agent (NOC-85); the owner reviews it with the PR, because it is CLI output developers see and RFC 0005 does not fix it
- Why: RFC 0005's "An example of each" summarizes an index as its fields and `equal N`, and shows no index with a scope. `ncdb model build` now adds `; scope space` the same way, naming the lock's key and value (`+ index task.inSpace (rank; scope space)`), and lists an index whose only change is its scope with `~`, since the build gives it the new model version; before, such a change raised the version without a line that said why.
- Instead of: listing no line for a change to the scope alone, as the report did before, which raises the model version without saying why; or a shorter word such as `; per space`, which names neither the option nor the lock key a developer would search for.
- Topics: cli, model-lock, rfc-0005, indexes
