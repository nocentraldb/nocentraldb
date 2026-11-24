# 2026-11-23 · NOC-502 · Confirming the recovery code asks for two of its words back, at positions the SDK picks at random

- Decided by: Carsten, 2026-11-23, on NOC-502 (option A)
- Why: forgetting the recovery code is the one step of an account that cannot be undone, and with a bare `confirm()` one tap was enough, so a person who never wrote the words down learned it only once every device was lost. Both SDKs now hand out two positions the SDK picked at random (`challenge()`), and `confirm(answers)` checks the words typed back at them, ignoring letter case and spacing as `recover` does. A wrong word fails with `invalidValue`, naming its position and never a word, and the code stays. The bare `confirm()` is gone: a breaking change, cheap before the beta. The SDK does not judge where the person stored the words, and ships no screen for the step.
- Instead of: adding the check beside a bare `confirm()` that an app could still call with one tap (B); only the guides suggesting that apps ask for words back (C).
- Topics: accounts, recovery, public-names, sdk-rust, sdk-ts, rfc-0010
