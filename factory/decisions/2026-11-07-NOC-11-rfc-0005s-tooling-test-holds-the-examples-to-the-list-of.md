# 2026-11-07 · NOC-11 · RFC 0005's tooling test holds the examples to the list of public names

- Decided by: coding agent (NOC-11)
- Why: the ticket asks the RFC to list every public name it introduces in one place for the owner; the test fails when an example uses an `nc.*` function, a modifier or an `ncdb model` command that the "Public names" section does not list, and when a JSON example does not parse.
- Instead of: checking only that the file exists and links to RFC 0001, which would let the list drift from the examples unnoticed.
- Topics: rfc-0005, schema, testing
