"""Writes the test vectors of RFC 0003's invitation
(docs/rfc/0003-keys-and-cryptography.md, "Invitation") in this directory,
independently of the Rust core and the SDKs: with cbor2 (6.2.1, canonical
mode) for the certificate map the account key signs, hashlib for the member
id, and cryptography's Ed25519 and X25519.

It reuses the keys of crates/ncdb-core/tests/vectors/membership/generate.py:
the account key 0x11..., and the member whose two secrets are 0x61... and
0x62..., certified as a device. `invitation` carries no account key
statement, as the SDK's own databases write today; `invitation-statement`
carries one, signed with number 1 by the recovery key of the same file,
derived from the recovery code 0xa0, 0xa1, ... 0xaf (RFC 0003, "Recovery
key").

Each vector is two files: `<name>.txt`, the invitation's text form, and
`<name>.hex`, its bytes. `fields.txt` lists the fields of both by name. The
gates do not run this script; the Rust SDK and scripts/rfc-0003-invitation.test.mjs
check the committed files.

Run: python3 -I generate.py  (needs cbor2==6.2.1 and cryptography==50.0.x;
Ed25519 signatures are deterministic, so a patch version changes no byte)
"""

import base64
import hashlib
import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

HERE = pathlib.Path(__file__).parent
PREFIX = "ncdb-invite-1:"
VERSION = 1


def enc(value):
    return cbor2.dumps(value, canonical=True)


def sha256(data):
    return hashlib.sha256(data).digest()


def raw(public_key):
    return public_key.public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )


ACCOUNT = Ed25519PrivateKey.from_private_bytes(bytes([0x11] * 32))
ACCOUNT_PUBLIC = raw(ACCOUNT.public_key())
RECOVERY_CODE = bytes(0xA0 + i for i in range(16))
RECOVERY_SEED = HKDF(
    algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/recovery-key"
).derive(RECOVERY_CODE)
RECOVERY = Ed25519PrivateKey.from_private_bytes(RECOVERY_SEED)
RECOVERY_PUBLIC = raw(RECOVERY.public_key())

SIGNING = raw(Ed25519PrivateKey.from_private_bytes(bytes([0x61] * 32)).public_key())
AGREEMENT = raw(X25519PrivateKey.from_private_bytes(bytes([0x62] * 32)).public_key())
MEMBER_ID = sha256(b"nocentraldb/member-id" + SIGNING + AGREEMENT)
KIND = 1  # a device

# RFC 0003, "Member certificate": the account key signs the SHA-256 hash of
# "nocentraldb/member-cert" and the canonical map of keys 1 to 3.
CERT_SIGNATURE = ACCOUNT.sign(
    sha256(b"nocentraldb/member-cert" + enc({1: ACCOUNT_PUBLIC, 2: MEMBER_ID, 3: KIND}))
)

# RFC 0003, "Account key statement": the recovery key signs the SHA-256 hash
# of "nocentraldb/account-key-statement" and the canonical map of keys 1 to 3.
NUMBER = 1
STATEMENT_SIGNATURE = RECOVERY.sign(
    sha256(
        b"nocentraldb/account-key-statement"
        + enc({1: RECOVERY_PUBLIC, 2: ACCOUNT_PUBLIC, 3: NUMBER})
    )
)


def invitation(statement):
    """RFC 0003, "Invitation", version 1."""
    data = bytes([VERSION]) + SIGNING + AGREEMENT + ACCOUNT_PUBLIC
    data += bytes([KIND]) + CERT_SIGNATURE
    if statement:
        data += bytes([1]) + RECOVERY_PUBLIC + NUMBER.to_bytes(8, "big")
        data += STATEMENT_SIGNATURE
    else:
        data += bytes([0])
    assert len(data) == (267 if statement else 163)
    return data


def text(data):
    return PREFIX + base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def write(name, data):
    hexed = data.hex()
    lines = [hexed[i : i + 64] for i in range(0, len(hexed), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")
    (HERE / f"{name}.txt").write_text(text(data) + "\n")


write("invitation", invitation(statement=False))
write("invitation-statement", invitation(statement=True))

fields = {
    "member id": MEMBER_ID,
    "signing key": SIGNING,
    "agreement key": AGREEMENT,
    "account key": ACCOUNT_PUBLIC,
    "kind": bytes([KIND]),
    "certificate signature": CERT_SIGNATURE,
    "recovery key": RECOVERY_PUBLIC,
    "number": NUMBER.to_bytes(8, "big"),
    "statement signature": STATEMENT_SIGNATURE,
}
(HERE / "fields.txt").write_text(
    "".join(f"{name}: {value.hex()}\n" for name, value in fields.items())
)
