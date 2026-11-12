"""Writes the test vectors of RFC 0003's sealed personal value
(docs/rfc/0003-keys-and-cryptography.md, "Personal value") and of the
operations that carry it (RFC 0004, "A personal value"; RFC 0005,
"Identifying the person") in this directory, independently of the Rust core
and the SDKs: with cbor2 (6.2.1, canonical mode) for the CBOR, cryptography's
HKDF-SHA256 for the subkey, and PyNaCl's XChaCha20-Poly1305 (libsodium) for
the AEAD. The owner decided the format on NOC-244 (Carsten, 2026-11-12, A to
all twelve questions).

The inputs are fixed placeholders: the space id 0x10, 0x11, ... 0x1f, the
person's object id 0x30 ... 0x3f, the person-key id 0x61 ... 0x70 and the
person key 0x55... of crates/ncdb-core/tests/vectors/membership/generate.py,
the person id 0x70 ... 0x7f, and the nonce 0xc1 ... 0xd8 (a real nonce is
random). The personal field is `name` of the collection `people`, written
"Anna Schmidt".

Files:
- personal-value.subkey.hex       HKDF-SHA256 of the person key, 32 bytes
- personal-value.ad.hex           the associated data
- personal-value.plaintext.hex    the canonical CBOR of the value
- personal-value.sealed.hex       nonce (24) || ciphertext || tag (16)
- personal-value.operation.hex    the set operation that writes it, a first
                                  write with an empty `replaces`
- personal-value.clear.hex        the set that clears it, plain null,
                                  replacing the first write
- person.operation.hex            the set of `$person` that names the
                                  person id and the person-key id

The gates do not run this script; scripts/rfc-personal-fields.test.mjs checks
the committed files, and the Rust core will.

Run: python3 -I personal_value.py  (needs cbor2==6.2.1, cryptography==50.0.x,
PyNaCl==1.6.2)
"""

import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from nacl.bindings import (
    crypto_aead_xchacha20poly1305_ietf_decrypt,
    crypto_aead_xchacha20poly1305_ietf_encrypt,
)

HERE = pathlib.Path(__file__).parent
LABEL = b"nocentraldb/personal-value"


def enc(value):
    return cbor2.dumps(value, canonical=True)


def name(text):
    """A collection or field name: its length, 2 bytes big-endian, then its
    UTF-8 bytes."""
    data = text.encode("utf-8")
    return len(data).to_bytes(2, "big") + data


SPACE = bytes(0x10 + i for i in range(16))
OBJECT = bytes(0x30 + i for i in range(16))
PERSON_KEY_ID = bytes(0x61 + i for i in range(16))
PERSON_KEY = bytes([0x55] * 32)
PERSON_ID = bytes(0x70 + i for i in range(16))
NONCE = bytes(0xC1 + i for i in range(24))
COLLECTION = "people"
FIELD = "name"
VALUE = "Anna Schmidt"

# Question 2: a subkey of the person key, HKDF-SHA256 with no salt and the
# info "nocentraldb/personal-value".
SUBKEY = HKDF(algorithm=hashes.SHA256(), length=32, salt=None, info=LABEL).derive(
    PERSON_KEY
)

# Question 1: the context, the space id, the object id and the person-key id
# (16 bytes each), then the collection and the field name, each with its
# length.
AD = LABEL + SPACE + OBJECT + PERSON_KEY_ID + name(COLLECTION) + name(FIELD)
assert len(AD) == 26 + 48 + 2 + 6 + 2 + 4

# Question 4: the canonical CBOR of the value, as argument 1 would carry it.
PLAINTEXT = enc(VALUE)

# Question 3: nonce (24) || ciphertext || tag (16).
SEALED = NONCE + crypto_aead_xchacha20poly1305_ietf_encrypt(PLAINTEXT, AD, NONCE, SUBKEY)
assert len(SEALED) == 24 + len(PLAINTEXT) + 16
assert (
    crypto_aead_xchacha20poly1305_ietf_decrypt(SEALED[24:], AD, SEALED[:24], SUBKEY)
    == PLAINTEXT
)

# Question 5: a set (op 1) whose argument 1 is the sealed personal value
# {1: person-key id, 2: sealed bytes}, and argument 2 `replaces`, readable.
OPERATION = {
    1: OBJECT,
    2: COLLECTION,
    3: FIELD,
    4: 1,
    5: {1: {1: PERSON_KEY_ID, 2: SEALED}, 2: []},
}

# Question 6: clearing writes a plain null, here replacing the first write,
# the operation at position 0 of a change whose id is 0xaa repeated.
FIRST_WRITE = [bytes([0xAA] * 32), 0]
CLEAR = {1: OBJECT, 2: COLLECTION, 3: FIELD, 4: 1, 5: {1: None, 2: [FIRST_WRITE]}}

# Question 7: the hidden field `$person` on the person's object, a set whose
# argument 1 names the person id and the person-key id in this space.
PERSON = {
    1: OBJECT,
    2: COLLECTION,
    3: "$person",
    4: 1,
    5: {1: {1: PERSON_ID, 2: PERSON_KEY_ID}, 2: []},
}

FILES = {
    "personal-value.subkey.hex": SUBKEY,
    "personal-value.ad.hex": AD,
    "personal-value.plaintext.hex": PLAINTEXT,
    "personal-value.sealed.hex": SEALED,
    "personal-value.operation.hex": enc(OPERATION),
    "personal-value.clear.hex": enc(CLEAR),
    "person.operation.hex": enc(PERSON),
}

for file, data in FILES.items():
    (HERE / file).write_text(data.hex() + "\n")
    print(f"{file}: {len(data)} bytes")
