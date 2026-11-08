# 2026-11-07 · NOC-14 · minicbor reads the CBOR tokens; the core checks the profile itself

- Decided by: coding agent (NOC-14)
- Why: RFC 0002's profile is stricter than any library's deterministic mode, so the core decodes into its own `Value`, rejects indefinite lengths, tags, other simple values, key types, unsorted and repeated keys and invalid UTF-8 as it reads, and then encodes again and compares, which catches every non-shortest integer, length and float and every other NaN. minicbor (`=2.3.1`, `alloc` only) supplies the tokens without serde and without `std`; floats are written by the core, because minicbor's half-precision support needs the `half` crate. The test vectors are written by `tests/vectors/generate.py` with Python's cbor2 and hashlib, independently of the crate; the gates check the committed vectors, they do not run the script.
- Instead of: ciborium with serde (its canonical output is not the profile, and the re-encode check would still be needed), or a hand-written tokenizer.
- Topics: cbor, change-format, dependencies
