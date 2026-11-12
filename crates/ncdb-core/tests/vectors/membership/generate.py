"""Writes the test vectors of RFC 0008's genesis, membership entries,
account key statement entry and erasure decision
(docs/rfc/0008-membership.md, "The control bodies") in this directory,
independently of the Rust core: with cbor2 (6.2.1, canonical mode) for the
bytes, hashlib for the hashes, cryptography's Ed25519 and HKDF, and PyNaCl's
XChaCha20-Poly1305 (libsodium) for the sealed model id, the encrypted
previous key of a key chain entry and a wrapped person key. It also writes
RFC 0003's recovery key, account key statement and the certificates that carry one as key 6. RFC
0008 gives no
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

# A key chain entry (entry type 3) by the creator after the add: generation
# 2 after the first, with generation 1's key encrypted as the owner decided
# in NOC-97 (Carsten, 2026-11-10): nonce (24) || ciphertext (32) || tag (16),
# XChaCha20-Poly1305 under RFC 0003's key-chain subkey of generation 2's key,
# with the space id, the new generation id and the previous generation id
# as associated data (each id 8 bytes, big-endian). It answers nothing, as
# a merge rotation after a member left does.
GENERATION_2_KEY = bytes([0x42] * 32)
CHAIN_NONCE = bytes(0x81 + i for i in range(24))
tag = sha256(b"nocentraldb/generation" + SPACE + CREATOR.id + enc([FIRST_GENERATION]))[:6]
SECOND_GENERATION = (2 << 48) | int.from_bytes(tag, "big")
chain_key = HKDF(
    algorithm=hashes.SHA256(),
    length=32,
    salt=None,
    info=b"nocentraldb/key-chain" + SPACE + SECOND_GENERATION.to_bytes(8, "big"),
).derive(GENERATION_2_KEY)
chain_ad = SPACE + SECOND_GENERATION.to_bytes(8, "big") + FIRST_GENERATION.to_bytes(8, "big")
PREVIOUS_FIELD = CHAIN_NONCE + crypto_aead_xchacha20poly1305_ietf_encrypt(
    GENERATION_1_KEY, chain_ad, CHAIN_NONCE, chain_key
)
assert len(PREVIOUS_FIELD) == 72
key_chain = {1: 3, 3: [ADD_ID], 8: SECOND_GENERATION, 9: [{1: FIRST_GENERATION, 2: PREVIOUS_FIELD}], 10: []}

# A person key wrapped under generation 2, as the owner decided in NOC-209
# (Carsten, 2026-11-11, A): nonce (24) || ciphertext (32) || tag (16),
# XChaCha20-Poly1305 under RFC 0003's person-key subkey of generation 2's
# key, HKDF-SHA256 with the info "nocentraldb/person-key-wrap" || space id
# || generation id (8 bytes, big-endian), and the space id, the generation
# id and the person-key id as associated data, 40 bytes.
PERSON_KEY_ID = bytes(0x61 + i for i in range(16))
PERSON_KEY = bytes([0x55] * 32)
WRAP_NONCE = bytes(0xA1 + i for i in range(24))
wrap_key = HKDF(
    algorithm=hashes.SHA256(),
    length=32,
    salt=None,
    info=b"nocentraldb/person-key-wrap" + SPACE + SECOND_GENERATION.to_bytes(8, "big"),
).derive(GENERATION_2_KEY)
wrap_ad = SPACE + SECOND_GENERATION.to_bytes(8, "big") + PERSON_KEY_ID
assert len(wrap_ad) == 40
PERSON_KEY_WRAPPED = WRAP_NONCE + crypto_aead_xchacha20poly1305_ietf_encrypt(
    PERSON_KEY, wrap_ad, WRAP_NONCE, wrap_key
)
assert len(PERSON_KEY_WRAPPED) == 72

# RFC 0003, "Recovery key": the seed is HKDF-SHA256 of the recovery code's
# 128 bits, with no salt and the info "nocentraldb/recovery-key", 32 bytes,
# used as the recovery key's Ed25519 secret.
RECOVERY_CODE = bytes(0xA0 + i for i in range(16))
RECOVERY_SEED = HKDF(
    algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/recovery-key"
).derive(RECOVERY_CODE)
RECOVERY = Ed25519PrivateKey.from_private_bytes(RECOVERY_SEED)
RECOVERY_PUBLIC = raw(RECOVERY.public_key())

# RFC 0003, "Account key statement": the recovery key signs the SHA-256
# hash of the context string and the canonical map of keys 1 to 3. This
# one names the account key above as the account's first.
statement_fields = {1: RECOVERY_PUBLIC, 2: ACCOUNT_PUBLIC, 3: 1}
STATEMENT = {
    **statement_fields,
    4: RECOVERY.sign(sha256(b"nocentraldb/account-key-statement" + enc(statement_fields))),
}

# RFC 0003, "Member certificate": key 6 carries the statement, as a map,
# outside key 5's signature, so the signature is the one without key 6. A
# sub-agent's certificate carries its parent's account's statement.
CREATOR_CERT_WITH_STATEMENT = {**CREATOR_CERT, 6: STATEMENT}
SUB_AGENT_CERT_WITH_STATEMENT = {**SUB_AGENT_CERT, 6: STATEMENT}

# RFC 0008, "Account key statement entry" (entry type 7; Carsten,
# 2026-11-11, NOC-187): after a lost device, the recovery key names a second
# account key with number 2, and a member carries that statement into the
# space. Key 8 holds the statement map as RFC 0003 defines it.
ACCOUNT_2 = Ed25519PrivateKey.from_private_bytes(bytes([0x12] * 32))
ACCOUNT_2_PUBLIC = raw(ACCOUNT_2.public_key())
statement_2_fields = {1: RECOVERY_PUBLIC, 2: ACCOUNT_2_PUBLIC, 3: 2}
STATEMENT_2 = {
    **statement_2_fields,
    4: RECOVERY.sign(sha256(b"nocentraldb/account-key-statement" + enc(statement_2_fields))),
}
statement_entry = {1: 7, 3: [ADD_ID], 8: STATEMENT_2}

# RFC 0008, "Erasure decision" (entry type 8; Carsten, 2026-11-11, NOC-230):
# key 8 names the provisional shred entry by its change id, key 9 says
# whether the decision finalizes it (true) or takes it back (false). The
# shred's change id is a placeholder here, 0x5d repeated; the decision
# depends on the shred, so key 3 lists it.
SHRED_ID = bytes([0x5D] * 32)
finalize = {1: 8, 3: [SHRED_ID], 8: SHRED_ID, 9: True}
take_back = {1: 8, 3: [SHRED_ID], 8: SHRED_ID, 9: False}

write("recovery-public", RECOVERY_PUBLIC)
write("statement", enc(STATEMENT))
write("certificate-with-statement", enc(CREATOR_CERT_WITH_STATEMENT))
write("sub-agent-certificate-with-statement", enc(SUB_AGENT_CERT_WITH_STATEMENT))
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
write("second-generation", SECOND_GENERATION.to_bytes(8, "big"))
write("key-chain-previous", PREVIOUS_FIELD)
write("key-chain.body", enc(key_chain))
write("person-key-wrapped", PERSON_KEY_WRAPPED)
write("statement-2", enc(STATEMENT_2))
write("statement-entry.body", enc(statement_entry))
write("erasure-decision-finalize.body", enc(finalize))
write("erasure-decision-take-back.body", enc(take_back))
