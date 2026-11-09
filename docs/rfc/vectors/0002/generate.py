"""Writes the byte-exact examples M1 and M2 of RFC 0002 Amendment 5
(docs/rfc/0002-change-and-log-format.md, "Byte-exact examples") in this
directory, from their descriptions in the RFC and independently of the Rust
core and of the tooling tests' reader: with cbor2 (6.2.1, canonical mode), as
docs/rfc/vectors/0004/generate.py does for RFC 0004 Amendment 1.
The gates do not run this script; scripts/rfc-0002-model.test.mjs checks
that the RFC's hex equals the committed files.

Run: python3 -I generate.py  (needs cbor2==6.2.1)

cbor2's canonical mode sorts map keys length first, RFC 0002 bytewise. The
two agree for the maps below (unsigned keys only).
"""

import datetime
import pathlib

import cbor2

HERE = pathlib.Path(__file__).parent

# The model of RFC 0005's example lock: model.id in hex, model.version 2.
MODEL_ID = bytes.fromhex("6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a")
MODEL_VERSION = 2

T1 = bytes([0x11] * 16)  # the object, as in RFC 0004's examples
A = bytes([0xAA] * 32)  # the one dependency, change id A

# The clock: 2026-11-09T00:00:00Z in the upper 48 bits, counter 0.
MILLIS = int(
    datetime.datetime(2026, 11, 9, tzinfo=datetime.timezone.utc).timestamp() * 1000
)
CLOCK = MILLIS << 16


def enc(value):
    return cbor2.dumps(value, canonical=True)


# M1. The value of payload key 64: {1: model id, 2: model version}.
MODEL = {1: MODEL_ID, 2: MODEL_VERSION}

# M2. A payload map that carries it, with RFC 0004's example E1 as its one
# operation and no padding.
PAYLOAD = {
    1: 1,
    2: CLOCK,
    3: [A],
    4: [{1: T1, 2: "tasks", 3: "title", 4: 1, 5: {1: "Ship v1", 2: []}}],
    64: MODEL,
}

EXAMPLES = {"M1": MODEL, "M2": PAYLOAD}


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


for name, value in EXAMPLES.items():
    data = enc(value)
    assert enc(cbor2.loads(data)) == data, name
    write(name, data)
