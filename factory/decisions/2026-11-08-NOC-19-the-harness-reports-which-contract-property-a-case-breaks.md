# 2026-11-08 · NOC-19 · The harness reports which contract property a case breaks, and shrinks within it

- Decided by: coding agent (NOC-19)
- Why: a broken fixture must be reported as the property it violates (C6), so the harness checks a case in a fixed order (C1 state bytes, C2 double application, C7 reads, C4 outcomes) and shrinks only towards cases that break the same property as the first failure. A divergence whose minimal case still needs a duplicate delivery is reported as C2, one that still needs a malformed operation as C5.
- Instead of: one pass/fail verdict, which cannot prove that each check fires on its own; or letting proptest shrink across properties, which reported a C1 divergence as a C2 double application on some seeds.
- Topics: convergence, testing
