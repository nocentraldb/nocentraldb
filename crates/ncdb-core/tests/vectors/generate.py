"""Writes the test vectors of RFC 0002 in this directory, independently of
the Rust core: with cbor2 (6.2.1, canonical mode) for the bytes and hashlib
for the change ids. The gates do not run this script; tests/change.rs checks
the committed files, natively and in wasm32.

Run: python3 -I generate.py  (needs cbor2==6.2.1)

cbor2's canonical mode sorts map keys length first, RFC 0002 bytewise. The
two agree for the maps below (unsigned keys only, or text keys only); do
not mix unsigned and text keys in one map here.
"""

import hashlib
import math
import pathlib

import cbor2

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
