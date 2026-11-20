# 2026-11-07 · NOC-6 · A change is deterministic CBOR, and an operation names its collection and field by their model.lock.json names

- Decided by: Carsten (RFC 0002 decisions D4 and D5)
- Why: deterministic CBOR is self-describing, so old clients can keep what they do not understand, and has maintained libraries in Rust and TypeScript. Names never change, need no id registry that two developers could fill differently, and let an old client tell which unknown field an operation is for; they cost a few bytes per operation, inside the ciphertext.
- Instead of: postcard or a hand-written format; numeric field ids that `ncdb model build` assigns.
- Topics: rfc-0002, encoding, change-format, model-lock

The profile is in `docs/rfc/0002-change-and-log-format.md`, "The CBOR profile"; the operation's keys in "Operations: object, field and value".
