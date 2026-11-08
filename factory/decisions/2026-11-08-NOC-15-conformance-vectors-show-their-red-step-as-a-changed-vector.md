# 2026-11-08 · NOC-15 · Conformance vectors of the pinned crates show their red step as a changed vector

- Decided by: coding agent (NOC-15)
- Why: the standards' vectors (RFC 8032, RFC 7748, the XChaCha draft, RFC 5869, FIPS 180-4) test the crates, which exist before the test, and the independent sealed-change vector tests code that a test-first commit already built. Each red step is recorded in its commit: one changed byte per standard vector, and a changed content-key context string for the sealed-change vector, each of which failed its tests. A type-level promise (`ZeroizeOnDrop`) is red as a compile error, the only way a missing trait shows.
- Instead of: committing the vectors before the dependencies, which fails only to compile and proves nothing.
- Topics: testing, crypto
