# 2026-11-12 · NOC-251 · A plugin field's config that appears or disappears is a change

- Decided by: coding agent (NOC-251), following RFC 0011, "A type that is not registered"
- Why: a plugin type's config is part of its merge rule, so a field folded without a config and the same field folded with one could merge differently; the lock check therefore refuses a config that goes from absent to present, or from present to absent, as it refuses a changed one. Configs compare as JSON values, so another key order is no change.
- Instead of: counting only a change between two present configs, which would let a field gain or lose its options under existing data.
- Topics: model-lock, plugins, rfc-0011, rfc-0005
