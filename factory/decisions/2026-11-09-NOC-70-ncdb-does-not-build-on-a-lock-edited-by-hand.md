# 2026-11-09 · NOC-70 · ncdb does not build on a lock edited by hand

- Decided by: coding agent (NOC-70)
- Why: `ncdb model build` raises the model version only when the model hash changes, and compares the new hash with the previous lock's `model.hash`. When that hash does not match the previous lock's content, someone edited the lock by hand, and building on it would number versions wrongly. Build and check stop and say to restore the lock from version control and change the schema instead.
- Instead of: recomputing the hash silently, which would hide the edit from review.
- Topics: cli, model-lock, rfc-0005
