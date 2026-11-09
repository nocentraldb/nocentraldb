# 2026-11-09 · NOC-67 · The membership vectors come from their own generator in the core's test vectors

- Decided by: coding agent (NOC-67)
- Why: RFC 0008 gives no byte-exact examples, so the ticket asks for a generator script. `crates/ncdb-core/tests/vectors/membership/generate.py` writes the certificates, the genesis (with its sealed model id), an add, set roles and a remove with cbor2 6.2.1, cryptography 50.0.2 and PyNaCl 1.6.2, the libraries of the core's other vectors, independently of the core; `tests/membership_body.rs` checks them natively and in wasm32. They sit next to the core's RFC 0002 vectors, not under `docs/rfc/vectors/`, because they are not the RFC's own examples.
- Instead of: adding examples to RFC 0008, which would change a decided RFC; or vectors written by the core itself, which would check the core against itself.
- Topics: membership, rfc-0008, testing, vectors
