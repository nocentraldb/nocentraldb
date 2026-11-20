# 2026-11-08 · NOC-23 · Space ids derive from the creator and a nonce; the genesis seals the model id; of two geneses the lower id wins

- Decided by: Carsten (RFC 0008 decisions M7, M8, M24 and M26)
- Why: with random space ids (RFC 0002's D3) anyone who learns a space id could write a competing genesis and take the space over; deriving it from the creator's member id and a random nonce keeps 128 unpredictable bits with no time in them, and object ids keep D3. The genesis carries the `model.id` (RFC 0005's D13 C) in key 13, sealed under a subkey of generation 1's key, so members can read it and the relay cannot learn which spaces share one app's model. Two geneses can come only from the creator's own device (a backup restore or clone); both stay in the log, as RFC 0002 keeps equivocations, and the one with the lower change id is the genesis.
- Instead of: keeping D3's 128 random bits with the lower genesis change id winning; the model id as 16 raw bytes readable by the relay, or as `H(space id || model.id)`, which members can only confirm, not read; rejecting both geneses and leaving the space unusable.
- Topics: rfc-0008, membership, model-lock, wire-format

The genesis fields and the sealed model id's bytes are in `docs/rfc/0008-membership.md`, "The first change of a space" and "Genesis".
