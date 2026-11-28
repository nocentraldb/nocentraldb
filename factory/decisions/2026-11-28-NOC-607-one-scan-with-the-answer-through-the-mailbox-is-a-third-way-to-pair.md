# 2026-11-28 · NOC-607 · One scan of the invitation, with the answer through the mailbox, is a third way to pair

- Decided by: Carsten, 2026-11-28, on NOC-607 (question 1, option A)
- Why: a device of the account scans the new device's invitation from its screen and sends RFC 0010's answer through the mailbox, sealed to the agreement key it scanned. The camera ties the pairing to the device in the person's hand, so nobody can put another device's request in its place and the person compares nothing; the relay can only withhold the answer. The new device posts no request: it tells the relay that it waits for an answer from the account the app names, and takes an answer only whose recovery key is that identity, so the relay cannot pair it into an account of its own. RFC 0007 gains `wait-pairing` and `send-pairing` for it. The answer carries the account key past the relay sealed, as NOC-597 decided; the two codes stay as the way in which it never does.
- Instead of: leaving it out, so that a person who can scan pairs with the two codes (B).
- Topics: pairing, relay, protocol, accounts, rfc-0019, rfc-0010
