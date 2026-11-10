# 2026-11-10 · NOC-97 · An encrypted previous key in a key chain entry is nonce, ciphertext and tag, 72 bytes

- Decided by: Carsten, 2026-11-10, on NOC-97
- Why: `nonce (24) || ciphertext (32) || tag (16)`, XChaCha20-Poly1305 with a random nonce under the key-chain subkey, is the layout of the sealed payload and of the genesis model id, so one layout serves everything a space key seals. NOC-106 writes it into RFC 0003's "Key-chain subkey" as decision 18; RFC 0008's key chain entry, merge rotations included, stores its previous keys this way.
- Instead of: a CBOR map `{1: nonce, 2: ciphertext and tag}`, about 76 bytes and a second layout to read; or a deterministic nonce, 48 bytes, which could repeat a nonce under one key when two rotations name the same generation.
- Topics: rfc-0003, rfc-0008, crypto, wire
