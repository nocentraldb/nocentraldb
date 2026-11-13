"""Writes the test vectors of RFC 0003's key record
(docs/rfc/0003-keys-and-cryptography.md, "The member key": "The key
record"; Carsten, 2026-11-12, NOC-285) in this directory, independently of
the Rust core and the SDKs: with cbor2 (6.2.1, canonical mode) for the maps
the keys sign, hashlib for the member id, and cryptography's Ed25519, X25519
and HKDF.

The secrets are test values, never keys of anyone: the member's two secrets
0x61... and 0x62..., the account secret 0x11..., and the recovery code 0xa0,
0xa1, ... 0xaf, as generate.py uses them. Each record is one file,
`key-record.<state>.hex`:

  format-1      format 1, 113 bytes: the member's secrets and the account
                secret, from before accounts were built
  pairing       format 2, 83 bytes: no account yet
  device        format 2, 219 bytes: the account key and statement number 1
  device-code   format 2, 235 bytes: the same, holding the recovery code
  agent         format 2, 283 bytes: an agent's certificate and the
                statement, no account key

The gates do not run this script; scripts/rfc-0003-key-record.test.mjs and
the Rust SDK check the committed files.

Run: python3 -I key_record.py  (needs cbor2==6.2.1 and cryptography==50.0.x;
Ed25519 signatures are deterministic, so a patch version changes no byte)
"""

import hashlib
import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

HERE = pathlib.Path(__file__).parent
MAGIC = b"nocentraldb-keys"


def enc(value):
    return cbor2.dumps(value, canonical=True)


def sha256(data):
    return hashlib.sha256(data).digest()


def raw(public_key):
    return public_key.public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)


SIGNING_SECRET = bytes([0x61] * 32)
AGREEMENT_SECRET = bytes([0x62] * 32)
ACCOUNT_SECRET = bytes([0x11] * 32)
RECOVERY_CODE = bytes(0xA0 + i for i in range(16))

SIGNING = raw(Ed25519PrivateKey.from_private_bytes(SIGNING_SECRET).public_key())
AGREEMENT = raw(X25519PrivateKey.from_private_bytes(AGREEMENT_SECRET).public_key())
MEMBER_ID = sha256(b"nocentraldb/member-id" + SIGNING + AGREEMENT)

ACCOUNT = Ed25519PrivateKey.from_private_bytes(ACCOUNT_SECRET)
ACCOUNT_PUBLIC = raw(ACCOUNT.public_key())
RECOVERY = Ed25519PrivateKey.from_private_bytes(
    HKDF(
        algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/recovery-key"
    ).derive(RECOVERY_CODE)
)
RECOVERY_PUBLIC = raw(RECOVERY.public_key())

NUMBER = 1
STATEMENT_SIGNATURE = RECOVERY.sign(
    sha256(
        b"nocentraldb/account-key-statement"
        + enc({1: RECOVERY_PUBLIC, 2: ACCOUNT_PUBLIC, 3: NUMBER})
    )
)
# An agent's certificate (kind 2), signed by the account key.
AGENT_CERT_SIGNATURE = ACCOUNT.sign(
    sha256(b"nocentraldb/member-cert" + enc({1: ACCOUNT_PUBLIC, 2: MEMBER_ID, 3: 2}))
)

MEMBER = MAGIC + bytes([2]) + SIGNING_SECRET + AGREEMENT_SECRET
STATEMENT = RECOVERY_PUBLIC + NUMBER.to_bytes(8, "big") + STATEMENT_SIGNATURE

records = {
    "format-1": MAGIC + bytes([1]) + SIGNING_SECRET + AGREEMENT_SECRET + ACCOUNT_SECRET,
    "pairing": MEMBER + bytes([0]) + bytes([0]),
    "device": MEMBER + bytes([1]) + ACCOUNT_SECRET + STATEMENT + bytes([0]),
    "device-code": MEMBER + bytes([1]) + ACCOUNT_SECRET + STATEMENT + bytes([1]) + RECOVERY_CODE,
    "agent": MEMBER + bytes([2]) + ACCOUNT_PUBLIC + AGENT_CERT_SIGNATURE + STATEMENT + bytes([0]),
}
sizes = {"format-1": 113, "pairing": 83, "device": 219, "device-code": 235, "agent": 283}
for name, data in records.items():
    assert len(data) == sizes[name], (name, len(data))
    hexed = data.hex()
    lines = [hexed[i : i + 64] for i in range(0, len(hexed), 64)]
    (HERE / f"key-record.{name}.hex").write_text("\n".join(lines) + "\n")

fields = {
    "member signing secret": SIGNING_SECRET,
    "member agreement secret": AGREEMENT_SECRET,
    "member id": MEMBER_ID,
    "account secret": ACCOUNT_SECRET,
    "account key": ACCOUNT_PUBLIC,
    "recovery code": RECOVERY_CODE,
    "recovery key": RECOVERY_PUBLIC,
    "number": NUMBER.to_bytes(8, "big"),
    "statement signature": STATEMENT_SIGNATURE,
    "agent certificate signature": AGENT_CERT_SIGNATURE,
}
(HERE / "key-record.fields.txt").write_text(
    "".join(f"{name}: {value.hex()}\n" for name, value in fields.items())
)
