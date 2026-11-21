# 2026-11-21 · NOC-438 · A reopened copy resolves its control changes once, and holds the space keys of the set, not of each step

- Decided by: coding agent (NOC-438)
- Why: `SpaceState::from_log`, and the view's migration to an extended model, gave the checks the log's folded control changes one at a time, and each one resolved the whole set again, so opening a space cost the square of its control changes on top of `resolve`'s own cost. `SealChecks::add_controls` (and `Checks::add_controls`) resolves the set once. Its control state, space keys, model and wrong envelopes equal those of adding the changes one by one, since they derive from the set. Only the keys a key chain entry gave before another entry replaced them are no longer held. `open_folded` needs those keys only for changes folded under them, and the log already un-folded such changes when the replacing entry arrived. Replaying the log never reproduced the order of arrival anyway.
- Instead of: replaying the control changes one by one to rebuild every key held on the way (one resolve per change), or persisting the held keys (an on-disk format change)
- Topics: membership, performance, keys
