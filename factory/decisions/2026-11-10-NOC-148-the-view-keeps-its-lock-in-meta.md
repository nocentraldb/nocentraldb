# 2026-11-10 · NOC-148 · The view keeps its lock in `meta` as canonical JSON, and a database from before opens only with its own model

- Decided by: coding agent (NOC-148); the owner reviews it with the PR, as it adds a key to the view's database
- Why: RFC 0005 decides that the SDK stores the lock in the local database, not how. The view keeps it under the key `lock` of its `meta` table, as the canonical JSON `ncdb model build` writes (`ModelLock::to_canonical_json`), in the transaction that opens the view; RFC 0006 (decision 2) leaves the view's tables internal, so the view's format version stays 2. A database written before this (NOC-108, alpha only) holds no lock: it opens only with a lock whose model folds its view as it is, and keeps that lock from then on; with any other it stays `ModelRefused`, as before.
- Instead of: a table of its own for the lock, or only its `model.hash`, which cannot run the additive check; raising the view's format version, which would refuse every existing database until rebuilds exist.
- What this does not do: a database from before that is reopened once with a lock that changes only `model.id` keeps that lock, since there is nothing to compare it with.
- Topics: sdk-rust, rfc-0005, rfc-0006, local-view, model-lock
