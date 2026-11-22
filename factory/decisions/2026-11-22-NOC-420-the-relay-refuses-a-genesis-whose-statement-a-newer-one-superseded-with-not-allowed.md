# 2026-11-22 · NOC-420 · The relay refuses a genesis whose statement a newer one superseded with not_allowed

- Decided by: the coding agent of NOC-420, for pm to confirm
- Why: RFC 0015 says the relay refuses such a genesis but names no error; the check decides whether the account may create spaces, which is what not_allowed means, and it is the answer RFC 0015's SDK already handles by fetching an admission, which then fails the same way. A statement of the same number counts as seen, so two devices of one account key create spaces alike.
- Instead of: membership_rule, which RFC 0008's fold gives an add of a superseded account key inside a space; a genesis is not checked against a space's rules.
- Topics: relay, accounts, admission, rfc-0015
