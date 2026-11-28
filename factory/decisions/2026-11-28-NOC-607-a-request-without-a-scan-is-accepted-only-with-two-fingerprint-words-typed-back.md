# 2026-11-28 · NOC-607 · A pairing request without a scan is accepted only with two of the new device's fingerprint words typed back

- Decided by: Carsten, 2026-11-28, on NOC-607 (question 2, option A)
- Why: `challenge()` names two random positions of the new device's 32 fingerprint words, the person types the words the new device shows there, and `accept(answers)` pairs only when both match, failing with `invalidValue` that names the position and never a word, as confirming the recovery code does (NOC-502). A request the relay or a stranger posted can then not be accepted with one tap, and the person has to look at the new device's screen. Two words check about 16 bits, not the full digest, so the full fingerprint stays on both screens and RFC 0019 says what the two words do not prove.
- Instead of: `challenge()` offered beside a one-tap `accept()`, which an app could skip (B); or no typed words, with the comparison left to the person (C).
- Topics: pairing, fingerprint, sdk, accounts, rfc-0019
