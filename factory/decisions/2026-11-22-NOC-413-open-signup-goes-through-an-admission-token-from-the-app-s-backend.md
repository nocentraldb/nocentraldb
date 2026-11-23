# 2026-11-22 · NOC-413 · Open signup goes through an admission token that the app's backend signs, and the relay records the identity at runtime

- Decided by: Carsten, 2026-11-22, on RFC 0015's questions, through pm (NOC-429)
- Why: the app's signup already fights fake accounts; the relay holds no secret and offers no privileged endpoint; the relay core checks the token the same way on every backend; a stolen token admits only its own identity.
- Instead of: a runtime list behind a privileged admin endpoint (B), which needs a credential in every app's backend and differs per backend; open signup with a rate limit (D), which cannot bound abuse because accounts are free to draw.
- Topics: relay, admission, aws, cost, rfc-0015
