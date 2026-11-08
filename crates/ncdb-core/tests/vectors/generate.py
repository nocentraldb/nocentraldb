"""Writes the test vectors of RFC 0002 in this directory, independently of
the Rust core: with cbor2 (6.2.1, canonical mode) for the bytes and hashlib
for the change ids. The gates do not run this script; tests/change.rs checks
the committed files, natively and in wasm32.

Run: python3 -I generate.py  (needs cbor2==6.2.1, cryptography==50.0.2 and
PyNaCl==1.6.2)

The signed, sealed change of RFC 0003 (NOC-15) is written with
cryptography's HKDF and Ed25519 and PyNaCl's XChaCha20-Poly1305 (libsodium),
from the derivation, associated data, nonce layout and padding that
src/seal.rs documents.

cbor2's canonical mode sorts map keys length first, RFC 0002 bytewise. The
two agree for the maps below (unsigned keys only, or text keys only); do
not mix unsigned and text keys in one map here.
"""

import hashlib
import math
import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_encrypt

HERE = pathlib.Path(__file__).parent


def enc(value):
    return cbor2.dumps(value, canonical=True)


def change(header, signature):
    header_bytes = enc(header)
    frame = enc([1, header_bytes, signature])
    change_id = hashlib.sha256(b"nocentraldb/change-id" + bytes([1]) + header_bytes).digest()
    return frame, change_id


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


SPACE = bytes(range(16))
AUTHOR = bytes(0xA0 + i for i in range(32))
OBJECT_A = bytes(0x30 + i for i in range(16))
OBJECT_B = bytes(0x50 + i for i in range(16))
CLOCK = (1_793_000_000_000 << 16) | 3

vectors = {
    "sealed-change": (
        {1: SPACE, 2: AUTHOR, 3: 1, 4: 0, 5: 0, 6: b"opaque ciphertext", 7: 1, 8: 1},
        bytes([0xEE] * 64),
    ),
    "control-change": (
        {1: SPACE, 2: AUTHOR, 3: 2, 4: 3, 5: 1, 6: bytes([0xA1, 0x01, 0x01]), 7: 1, 8: 1},
        bytes(range(64)),
    ),
    "newer-header": (
        {
            1: SPACE,
            2: AUTHOR,
            3: 3,
            4: 0,
            5: 0,
            6: b"opaque ciphertext",
            7: 2,
            8: 1,
            9: "must understand",
            64: [1, -1, 1.5],
            1000: {"note": None},
        },
        bytes([0xEE] * 64),
    ),
}

for name, (header, signature) in vectors.items():
    frame, change_id = change(header, signature)
    write(f"{name}.frame", frame)
    write(f"{name}.id", change_id)

DEPS = sorted([hashlib.sha256(b"a").digest(), hashlib.sha256(b"b").digest()])

payload = {
    1: 1,
    2: CLOCK,
    3: DEPS,
    4: [
        {1: OBJECT_A, 2: "tasks", 4: 0},
        {1: OBJECT_A, 2: "tasks", 3: "title", 4: 1, 5: "Write the RFC"},
        {
            1: OBJECT_A,
            2: "tasks",
            3: "estimate",
            4: 1,
            5: {
                "high": 1.5,
                "low": -0.0,
                "nan": math.nan,
                "max": 100000.0,
                "pi": 1234.5678,
                "n": -1000,
            },
        },
        {1: OBJECT_B, 2: "people", 3: "email", 4: 1, 5: {1: bytes([0x77] * 16), 2: b"sealed"}},
    ],
    5: bytes(4),
}
write("payload", enc(payload))

newer_payload = {
    1: 2,
    2: CLOCK + 1,
    3: [],
    4: [{1: OBJECT_A, 2: "c", 3: "f", 4: 9, 5: True, 6: "must", 70: False}],
    64: None,
}
write("newer-payload", enc(newer_payload))


# A signed, sealed change (RFC 0003, "How a member signs and encrypts a
# change"): the payload above without its padding, padded again to a
# multiple of 256 bytes, sealed and signed by the member of these secrets.
SIGNING_SECRET = bytes(0x10 + i for i in range(32))
AGREEMENT_SECRET = bytes(0x60 + i for i in range(32))
SPACE_KEY = bytes(0xC0 + i for i in range(32))
NONCE = bytes(0x90 + i for i in range(24))
SEQUENCE = 7
GENERATION = 2
SUITE = 1


def raw(public_key):
    return public_key.public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)


signing_key = Ed25519PrivateKey.from_private_bytes(SIGNING_SECRET)
signing_public = raw(signing_key.public_key())
agreement_public = raw(X25519PrivateKey.from_private_bytes(AGREEMENT_SECRET).public_key())
member_id = hashlib.sha256(b"nocentraldb/member-id" + signing_public + agreement_public).digest()

unpadded = {key: value for key, value in payload.items() if key != 5}
plaintext = enc(unpadded)
if len(plaintext) % 256:
    zeros = 0
    while len(enc({**unpadded, 5: bytes(zeros)})) % 256:
        zeros += 1
    plaintext = enc({**unpadded, 5: bytes(zeros)})

info = (
    b"nocentraldb/content-key"
    + SPACE
    + GENERATION.to_bytes(8, "big")
    + SUITE.to_bytes(8, "big")
)
content_key = HKDF(algorithm=hashes.SHA256(), length=32, salt=None, info=info).derive(SPACE_KEY)
header = {1: SPACE, 2: member_id, 3: SEQUENCE, 4: GENERATION, 5: 0, 7: 1, 8: SUITE}
associated_data = enc(header)
ciphertext = crypto_aead_xchacha20poly1305_ietf_encrypt(plaintext, associated_data, NONCE, content_key)
header[6] = NONCE + ciphertext
header_bytes = enc(header)
signature = signing_key.sign(header_bytes)
frame, change_id = change(header, signature)
write("signed-sealed-change.frame", frame)
write("signed-sealed-change.id", change_id)
write("signed-sealed-change.plaintext", plaintext)
write("signed-sealed-change.public", signing_public + agreement_public)
