# 2026-11-12 · NOC-285 · A device waiting to be paired shows invitation version 2, without a certificate, under the same prefix

- Decided by: Carsten (2026-11-12, on NOC-285, point 1, option B)
- Why: RFC 0010 says a database opened for pairing shows its invitation without a certificate, but RFC 0003's invitation layout always has one and readers refuse anything shorter. Version 2 under `ncdb-invite-1:` is version, signing key, agreement key and kind: 66 bytes, about 90 characters. Only `pair` accepts it; `members.add` refuses it with `invalidCertificate`, because a member without a certificate cannot join a space. Version 1 stays exactly as decided, and test vectors for version 2 are added beside it.
- Instead of: keeping version 1 with a new byte after the kind byte saying whether a certificate follows (A), which changes the decided version 1 layout so that readers already written would refuse the new form.
- Topics: pairing, invitations, accounts, rfc-0003, rfc-0010
