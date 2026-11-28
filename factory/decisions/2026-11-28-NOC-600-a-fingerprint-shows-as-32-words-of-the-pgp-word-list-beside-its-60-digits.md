# 2026-11-28 · NOC-600 · A fingerprint shows as 32 words of the PGP word list, one per byte of its full digest, beside its 60 digits

- Decided by: Carsten, 2026-11-28, on NOC-600 (option A)
- Why: the PGP word list was made for reading digests aloud; one word per byte shows all 256 bits of the SHA-256 digest, not a shorter code, and its two alternating lists catch swapped, repeated and dropped words. It is not the recovery code's BIP-39 list; 59 of its 512 words also stand in BIP-39, which RFC 0019 states, and a fingerprint is always 32 words in eight blocks of four.
- Instead of: 43 emojis of the Matrix protocol's 64-emoji list, which platforms draw differently (B); both words and emojis, with two lists to ship and apps that may pick differently (C); or 25 words, the digits' strength, which is not the full digest (D).
- Topics: pairing, fingerprint, accounts, rfc-0019, rfc-0010
