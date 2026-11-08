# 2026-11-07 · NOC-14 · Foldable means format 1, a known kind, and no unknown key from 1 to 63

- Decided by: coding agent (NOC-14)
- Why: RFC 0002 says a higher header or payload format is stored, not folded; this core knows only format 1, and no lower format exists, so any other number is not foldable. The padding (payload key 5) is kept as given and not checked for zeros, since "the reader ignores" it, and a header whose author member id is not 32 bytes is rejected, since RFC 0002 defines it as RFC 0003's SHA-256 member id. Whether op 0 carries no field is left to the fold, not the codec.
- Instead of: rejecting non-zero padding or accepting member ids of any length.
- Topics: change-format, log
