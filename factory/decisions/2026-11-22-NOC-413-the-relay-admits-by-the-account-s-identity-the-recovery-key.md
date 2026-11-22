# 2026-11-22 · NOC-413 · The relay admits by the account's identity, the recovery key, with the account key for an account without one

- Decided by: Carsten, 2026-11-22, on RFC 0015's questions, through pm (NOC-429)
- Why: recovery from the code and removeMember give an account a new account key under the same recovery key, and a list keyed by account key then refuses the account's devices with not_allowed. The certificate's statement already carries the recovery key and verifies under it. The relay keeps the highest statement number per identity, so a lost device's older certificate is refused once a newer one reached it. NOC-420 builds it.
- Instead of: keeping the list keyed by account key, which breaks every recovered account.
- Topics: relay, accounts, admission, rfc-0015
