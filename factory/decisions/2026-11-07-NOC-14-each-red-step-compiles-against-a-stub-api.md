# 2026-11-07 · NOC-14 · Each red step compiles against a stub API

- Decided by: coding agent (NOC-14)
- Why: a Rust test that calls a function that does not exist fails to compile, which is the "missing import" failure `roles/coder.md` excludes. The first commit adds the types and signatures with `todo!()` bodies, so each red commit shows its tests failing at run time with "not yet implemented", or for the behaviour (the derived `Debug` of the redaction step).
- Instead of: red commits that fail to compile.
- Topics: testing, process
