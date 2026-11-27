# 2026-11-27 · NOC-462 · The read-version cache keeps each lease's updates for its field, and refuses a lease whose clock the version does not reach

- Decided by: coding agent (NOC-462)
- Why: RFC 0013 makes an edit at the version read "plus the edits this agent's lease has written since", so the lease's clock continues (F13). The cache keeps every update a lease wrote to a field for as long as it holds a version of that field, and applies all of them to the version; Yjs applies an update it already holds once, so this equals the edits since the read. When the copy still holds fewer of the lease's items than the lease's clock, a writer the cache does not know used the lease, and a new item could reuse an id: the edit is refused. The cache takes its time limit and its clock from the caller, in milliseconds, so it runs the same natively and in wasm.
- Instead of: keeping updates per version, which misses an edit made through another version of the same read; continuing from whatever clock the version holds, which can write one item id twice.
- Topics: rich-text, rfc-0013, agents, convergence

The length of the time limit waits for NOC-566.
