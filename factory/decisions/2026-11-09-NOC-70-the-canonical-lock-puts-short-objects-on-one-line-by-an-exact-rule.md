# 2026-11-09 · NOC-70 · The canonical lock puts short objects on one line by an exact rule

- Decided by: coding agent (NOC-70)
- Why: RFC 0005 says the lock puts "short objects on one line as a formatter would put them", which two builds must apply identically. The rule: an array goes on one line when the line fits in 80 columns, its trailing comma included; an object does too, and only when none of its values is a non-empty object. It reproduces RFC 0005's example lock byte for byte, except that the example lists `notesAboutAssignee` before `dependsOn` against its own sorted key order; the builder sorts, and the RFC's model hash, computed from canonical JSON, is reproduced exactly. The RFC's example deserves an erratum.
- Instead of: running an external formatter at build time, which would make the bytes depend on its version.
- Topics: model-lock, rfc-0005, canonical-form
