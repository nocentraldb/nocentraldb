"""Writes the byte-exact examples of RFC 0002
(docs/rfc/0002-change-and-log-format.md) in this directory: M1 and M2 ("The
writer's model") and N1 and N2 ("The CBOR profile") of its "Byte-exact
examples", each from its description in the RFC and independently of the Rust
core and of the tooling tests' reader: with cbor2 (6.2.1, canonical mode), as
docs/rfc/vectors/0004/generate.py does for RFC 0004. The gates do not run this
script; scripts/rfc-0002-rules.test.mjs checks that the RFC's hex equals the
committed files, and crates/ncdb-core/tests/depth.rs decodes N1 and N2.

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


def nested(levels):
    """A value `levels` deep: arrays of one item around a 0."""
    value = 0
    for _ in range(levels - 1):
        value = [value]
    return value


def deep_payload(levels):
    """M2's payload with an operation whose value nests `levels` deep.

    The frame is level 1, the header map 2, the payload map 3, the
    operations array 4, the operation map 5 and its value 6, so a value
    27 levels deep reaches level 32, the limit, and 28 levels reach 33.
    """
    operation = {1: T1, 2: "tasks", 3: "title", 4: 1, 5: nested(levels)}
    return {1: 1, 2: CLOCK, 3: [A], 4: [operation], 64: MODEL}


EXAMPLES = {
    "M1": MODEL,
    "M2": PAYLOAD,
    # N1. A payload whose deepest item sits at level 32: accepted.
    "N1": deep_payload(27),
    # N2. The same, one level deeper, at level 33: refused.
    "N2": deep_payload(28),
}


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


for name, value in EXAMPLES.items():
    data = enc(value)
    assert enc(cbor2.loads(data)) == data, name
    write(name, data)
