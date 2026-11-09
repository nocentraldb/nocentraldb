# 2026-11-09 · NOC-68 · A key envelope has empty associated data, and the core reads its four body fields alone

- Decided by: coding agent (NOC-68), reported to pm
- Why: RFC 0003 ("Key envelope") defines the HPKE info and no associated data, so the core seals with an empty AAD; the info already binds the space id, the generation id and the recipient's member id, and the control change's signature covers the whole body. The ticket's "a changed AAD fails to open" is tested as a changed space, generation or recipient. NOC-67 had not defined a control-body type, so, as pm said, `KeyEnvelope::fields` and `from_fields` write and read only keys 8 to 11 of RFC 0008's entry type 2 and leave the other keys to the control body; placing them is the wiring ticket's. The envelope's recipient field must equal the opener's member id before HPKE runs, so an envelope for someone else fails as `WrongRecipient`, and a forged recipient field fails in HPKE as `Open`.
- Instead of: a non-empty AAD, which would change a decided wire format and is the owner's; or a stand-alone CBOR encoding of the envelope, which no RFC defines.
- Topics: crypto, rfc-0003, rfc-0008, wire-format
