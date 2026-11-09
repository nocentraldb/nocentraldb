# 2026-11-09 · NOC-67 · `resolve` lives in `ncdb-core` and verifies signatures with keys from the control set

- Decided by: coding agent (NOC-67)
- Why: the ticket asks that the relay and the members run the same function (RFC 0007, "What the relay checks"), and `ncdb-core` is the crate both build on, natively and in wasm32; so `ncdb_core::membership::resolve(space, changes)` takes the frames of a space and judges every control change itself. RFC 0008 folds "the set of control changes ... each with a valid signature", and the only keys a holder can check them with are the ones the genesis and the add entries name, which hash to the member id; so `resolve` verifies each signature with those keys, and rejects a change whose author no held entry names ("unknown author") or whose signature fails. The verdict is a function of the set, so it changes when the add that names the keys arrives.
- Instead of: a separate crate for membership, which the relay and the core would both depend on for one module; or verifying signatures before `resolve` through `Checks`, which would make the relay and the members bring the same keys from somewhere else.
- Topics: membership, rfc-0008, relay, core
