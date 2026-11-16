# 2026-11-15 · NOC-326 · An admin of the performer's account takes a provisional erasure back

- Decided by: Carsten, 2026-11-15, on NOC-326 (Q7 a); the notice text: pm, NOC-326, on the coding agent's proposal
- Why: RFC 0003 says a provisional erasure can be taken back within its 7 days, and nothing in the SDK did. `space.erasures().take_back(shred)` writes the erasure decision that takes it back (RFC 0008, "Erasure decision"). Only an admin of the performer's account may, while the erasure is provisional; any other call is refused. It returns the notice `NoticeCode::TakenBack` with the text "The erasure is taken back in this space, and the person's fields read again; a finalize another device of the account wrote before it saw this take-back still wins, and the person stays erased."
- Instead of: no take-back call, which would leave RFC 0003's "can be taken back" to an app that writes control changes itself.
- Topics: sdk-rust, erasure, notices
