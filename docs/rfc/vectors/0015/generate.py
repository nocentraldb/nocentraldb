"""Writes the test vectors of RFC 0015's admission
(docs/rfc/0015-relay-admission.md, "The admission") in this directory,
independently of the Rust core and the SDKs: with cbor2 (6.2.1, canonical
mode) for the map, hashlib for the hash the admission key signs, and
cryptography's HKDF and Ed25519.

An admission is a canonical CBOR map of key 1, the relay id; key 2, the
identity; key 3, until, in Unix seconds; key 4, the spaces the identity may
create; and key 5, the admission key's Ed25519 signature over the SHA-256
hash of "nocentraldb/relay-admission" and the canonical map of keys 1 to 4,
RFC 0003's rule for every signed input. As text it is `ncdb-admit-1:` and
the map's bytes in unpadded base64url.

The admission key is the Ed25519 key whose secret is 32 bytes 0x5a. The
relay id is the 16 bytes 0x00 to 0x0f. `admission` admits the identity of
an account with a recovery key: the recovery key of RFC 0003's vectors,
derived from the recovery code 0xa0, 0xa1, ... 0xaf (RFC 0003, "Recovery
key"). `admission-account-key` admits an account without one: its identity
is its account key, the key whose secret is 32 bytes 0x11 (RFC 0015, "The
account's identity").

Each vector is two files: `<name>.txt`, the text form, and `<name>.hex`, the
map's bytes. `fields.txt` lists the inputs and what the key signs by name.
The gates do not run this script; ncdb-core's tests/admission_vector.rs and
scripts/rfc-0015-admission.test.mjs check the committed files.

Run: python3 -I generate.py  (needs cbor2==6.2.1 and cryptography==50.0.x;
Ed25519 signatures are deterministic, so a patch version changes no byte)
"""

import base64
import hashlib
import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

HERE = pathlib.Path(__file__).parent
PREFIX = "ncdb-admit-1:"
CONTEXT = b"nocentraldb/relay-admission"


def enc(value):
    return cbor2.dumps(value, canonical=True)


def sha256(data):
    return hashlib.sha256(data).digest()


def raw(public_key):
    return public_key.public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )


ADMISSION_KEY = Ed25519PrivateKey.from_private_bytes(bytes([0x5A] * 32))
ADMISSION_PUBLIC = raw(ADMISSION_KEY.public_key())
RELAY_ID = bytes(range(16))

RECOVERY_CODE = bytes(0xA0 + i for i in range(16))
RECOVERY_SEED = HKDF(
    algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/recovery-key"
).derive(RECOVERY_CODE)
RECOVERY_PUBLIC = raw(Ed25519PrivateKey.from_private_bytes(RECOVERY_SEED).public_key())
ACCOUNT_PUBLIC = raw(Ed25519PrivateKey.from_private_bytes(bytes([0x11] * 32)).public_key())

UNTIL = 1798761600  # 2027-01-01T00:00:00Z
SPACES = 20


def admission(identity, until, spaces):
    """RFC 0015, "The admission": keys 1 to 4, then the signature as key 5."""
    unsigned = {1: RELAY_ID, 2: identity, 3: until, 4: spaces}
    digest = sha256(CONTEXT + enc(unsigned))
    return enc({**unsigned, 5: ADMISSION_KEY.sign(digest)}), digest


def write(name, data):
    (HERE / f"{name}.hex").write_text(data.hex() + "\n")
    text = PREFIX + base64.urlsafe_b64encode(data).decode().rstrip("=")
    (HERE / f"{name}.txt").write_text(text + "\n")


ADMISSION, DIGEST = admission(RECOVERY_PUBLIC, UNTIL, SPACES)
ACCOUNT_KEY_ADMISSION, ACCOUNT_KEY_DIGEST = admission(ACCOUNT_PUBLIC, UNTIL, 1)
write("admission", ADMISSION)
write("admission-account-key", ACCOUNT_KEY_ADMISSION)

fields = {
    "admission key": ADMISSION_PUBLIC,
    "relay id": RELAY_ID,
    "recovery key": RECOVERY_PUBLIC,
    "account key": ACCOUNT_PUBLIC,
    "until": UNTIL.to_bytes(8, "big"),
    "spaces": SPACES.to_bytes(8, "big"),
    "digest": DIGEST,
    "account key digest": ACCOUNT_KEY_DIGEST,
}
(HERE / "fields.txt").write_text(
    "".join(f"{name}: {value.hex()}\n" for name, value in fields.items())
)
