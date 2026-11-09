"""Writes the test vectors of RFC 0008's genesis and membership entries
(docs/rfc/0008-membership.md, "The control bodies") in this directory,
independently of the Rust core: with cbor2 (6.2.1, canonical mode) for the
bytes, hashlib for the hashes, cryptography's Ed25519 and HKDF, and PyNaCl's
XChaCha20-Poly1305 (libsodium) for the sealed model id. RFC 0008 gives no
byte-exact examples, so these follow its text and RFC 0003's "Formats of
what keys sign and derive". The gates do not run this script;
tests/membership_body.rs checks the committed files, natively and in wasm32.

Run: python3 -I generate.py  (needs cbor2==6.2.1, cryptography==50.0.2 and
PyNaCl==1.6.2)

cbor2's canonical mode sorts map keys length first, RFC 0002 bytewise. The
two agree for the maps below (unsigned keys only).
"""

import hashlib
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


def sha256(data):
    return hashlib.sha256(data).digest()


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


def raw(public_key):
    return public_key.public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)


class Member:
    """A member key from its two secrets (RFC 0003, "The member key")."""

    def __init__(self, signing_secret, agreement_secret):
        self.signing = Ed25519PrivateKey.from_private_bytes(signing_secret)
        self.signing_public = raw(self.signing.public_key())
        self.agreement_public = raw(
            X25519PrivateKey.from_private_bytes(agreement_secret).public_key()
        )
        self.id = sha256(b"nocentraldb/member-id" + self.signing_public + self.agreement_public)


def certificate(signer, context, account_public, member_id, kind, parent=None):
    """RFC 0003, "Member certificate": the signer signs the SHA-256 hash of
    the context string and the canonical map of keys 1 to 4."""
    fields = {1: account_public, 2: member_id, 3: kind}
    if parent is not None:
        fields[4] = parent
    signature = signer.sign(sha256(context + enc(fields)))
    return {**fields, 5: signature}


# The keys. Each secret is a run of one byte, so a reader can rebuild them.
ACCOUNT = Ed25519PrivateKey.from_private_bytes(bytes([0x11] * 32))
ACCOUNT_PUBLIC = raw(ACCOUNT.public_key())
CREATOR = Member(bytes([0x21] * 32), bytes([0x22] * 32))
SUBJECT = Member(bytes([0x61] * 32), bytes([0x62] * 32))
SUB_AGENT = Member(bytes([0x71] * 32), bytes([0x72] * 32))
NONCE = bytes(0x31 + i for i in range(16))
GENERATION_1_KEY = bytes([0x41] * 32)
MODEL_NONCE = bytes(0x51 + i for i in range(24))
# The model of RFC 0005's example lock.
MODEL_ID = bytes.fromhex("6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a")

# RFC 0008, "What it names": the space id is the first 16 bytes of
# H("nocentraldb/space-id" || creator member id || nonce).
SPACE = sha256(b"nocentraldb/space-id" + CREATOR.id + NONCE)[:16]

# RFC 0008, "Generation ids": ordinal 1 in the upper 16 bits, and the first
# 48 bits of H("nocentraldb/generation" || space id || author member id ||
# canonical array of the previous generation ids) below; the genesis has no
# previous generations.
tag = sha256(b"nocentraldb/generation" + SPACE + CREATOR.id + enc([]))[:6]
FIRST_GENERATION = (1 << 48) | int.from_bytes(tag, "big")

# RFC 0008, "Genesis", key 13 (decision M26 B), and RFC 0003, "Model-id
# subkey".
model_key = HKDF(
    algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/model-id" + SPACE
).derive(GENERATION_1_KEY)
model_ad = b"nocentraldb/model-id" + SPACE + CREATOR.id
MODEL_FIELD = MODEL_NONCE + crypto_aead_xchacha20poly1305_ietf_encrypt(
    MODEL_ID, model_ad, MODEL_NONCE, model_key
)
assert len(MODEL_FIELD) == 56

CREATOR_CERT = certificate(ACCOUNT, b"nocentraldb/member-cert", ACCOUNT_PUBLIC, CREATOR.id, 2)
SUBJECT_CERT = certificate(ACCOUNT, b"nocentraldb/member-cert", ACCOUNT_PUBLIC, SUBJECT.id, 1)
# A sub-agent of the creator, an agent: its parent's signing key signs it,
# and key 1 carries the parent's account key.
SUB_AGENT_CERT = certificate(
    CREATOR.signing,
    b"nocentraldb/sub-agent-cert",
    ACCOUNT_PUBLIC,
    SUB_AGENT.id,
    3,
    CREATOR.id,
)

genesis = {
    1: 0,
    3: [],
    8: NONCE,
    9: CREATOR.signing_public,
    10: CREATOR.agreement_public,
    11: CREATOR_CERT,
    12: FIRST_GENERATION,
    13: MODEL_FIELD,
}
genesis_body = enc(genesis)
# RFC 0002's header of a control change: sequence 1, the first generation,
# kind 1, header format 1, suite 1.
genesis_header = enc(
    {1: SPACE, 2: CREATOR.id, 3: 1, 4: FIRST_GENERATION, 5: 1, 6: genesis_body, 7: 1, 8: 1}
)
GENESIS_ID = sha256(b"nocentraldb/change-id" + bytes([1]) + genesis_header)
genesis_frame = enc([1, genesis_header, CREATOR.signing.sign(genesis_header)])

# Membership entries (entry type 1), each after the genesis.
add = {
    1: 1,
    3: [GENESIS_ID],
    8: 1,
    9: SUBJECT.id,
    10: 2,
    11: SUBJECT.signing_public,
    12: SUBJECT.agreement_public,
    13: SUBJECT_CERT,
    14: FIRST_GENERATION,
}
add_body = enc(add)
add_header = enc(
    {1: SPACE, 2: CREATOR.id, 3: 2, 4: FIRST_GENERATION, 5: 1, 6: add_body, 7: 1, 8: 1}
)
ADD_ID = sha256(b"nocentraldb/change-id" + bytes([1]) + add_header)
add_frame = enc([1, add_header, CREATOR.signing.sign(add_header)])

# Set role to admin, with a from generation, since admin writes; cutoff 4.
set_role = {1: 1, 3: [ADD_ID], 8: 2, 9: SUBJECT.id, 10: 3, 14: FIRST_GENERATION, 15: 4}
# Set role to reader: no from generation; cutoff 0, none held.
set_reader = {1: 1, 3: [ADD_ID], 8: 2, 9: SUBJECT.id, 10: 1, 15: 0}
# Remove, with cutoff 7.
remove = {1: 1, 3: [ADD_ID], 8: 3, 9: SUBJECT.id, 15: 7}

write("certificate", enc(CREATOR_CERT))
write("sub-agent-certificate", enc(SUB_AGENT_CERT))
write("space-id", SPACE)
write("first-generation", FIRST_GENERATION.to_bytes(8, "big"))
write("model-id-field", MODEL_FIELD)
write("genesis.body", genesis_body)
write("genesis.frame", genesis_frame)
write("genesis.id", GENESIS_ID)
write("add.body", add_body)
write("add.frame", add_frame)
write("add.id", ADD_ID)
write("set-role-admin.body", enc(set_role))
write("set-role-reader.body", enc(set_reader))
write("remove.body", enc(remove))
