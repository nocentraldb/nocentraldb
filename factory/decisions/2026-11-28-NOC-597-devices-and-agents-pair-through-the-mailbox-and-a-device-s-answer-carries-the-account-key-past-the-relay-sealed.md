# 2026-11-28 · NOC-597 · Devices and agents pair through the mailbox, and a device's answer carries the account key past the relay, sealed

- Decided by: Carsten, 2026-11-28, on NOC-597 (option A)
- Why: a device's pairing answer carries the account key sealed with HPKE to the new device, inside a second seal to the same key, so the relay and its backups hold it only as ciphertext; that ciphertext is not post-quantum, as every key envelope at the relay is not, and RFC 0019 says so plainly. The two codes stay for a person who wants the account key never to reach the relay.
- Instead of: a mailbox for agents only, whose answers carry no account key, which leaves new phones and laptops to a scan or copied text (B); or the mailbox for devices only where an app or relay opts in, a switch whose effect is the same as an app not calling the mailbox (C).
- Topics: pairing, crypto, relay, accounts, rfc-0019
