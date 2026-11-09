"""Writes the key-envelope test vector of RFC 0003 ("Key envelope") in this
directory, independently of the Rust core and of the `hpke` crate. The gates
do not run this script; tests/key_envelope_vector.rs checks the committed
files, natively and in wasm32.

Run: python3 -I key_envelope.py  (needs cbor2==6.2.1 and cryptography==50.0.1)

HPKE (RFC 9180) base mode with the suite RFC 0003 names, DHKEM(X25519,
HKDF-SHA256), HKDF-SHA256 and ChaCha20-Poly1305, is written out below from
cryptography's X25519, HMAC-SHA256 and ChaCha20-Poly1305, so that the
ephemeral key can be derived from a fixed ikmE. The script first checks that
this code reproduces RFC 9180's own vector A.2.1 (sequence number 0), then
seals the envelope, and finally opens the envelope with cryptography's own
HPKE (`cryptography.hazmat.primitives.hpke`), a second implementation.

The envelope: the HPKE info is "nocentraldb/key-envelope" || space id (16
bytes) || generation id (8 bytes, big-endian) || recipient member id (32
bytes); the plaintext is the 32-byte space key; RFC 0003 defines no
associated data, so it is empty. RFC 0008's entry type 2 carries the
recipient (key 8), the generation (key 9), HPKE's enc (key 10) and the
ciphertext (key 11); key-envelope.fields.hex is the canonical CBOR map of
just those four keys.
"""

import hashlib
import hmac
import pathlib

import cbor2
from cryptography.hazmat.primitives import hpke, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

HERE = pathlib.Path(__file__).parent
RAW = serialization.Encoding.Raw, serialization.PublicFormat.Raw

# The suite: KEM 0x0020, KDF 0x0001, AEAD 0x0003 (RFC 9180, section 7).
KEM_ID, KDF_ID, AEAD_ID = 0x0020, 0x0001, 0x0003
KEM_SUITE = b"KEM" + KEM_ID.to_bytes(2, "big")
HPKE_SUITE = b"HPKE" + KEM_ID.to_bytes(2, "big") + KDF_ID.to_bytes(2, "big") + AEAD_ID.to_bytes(2, "big")


def extract(salt, ikm):
    return hmac.new(salt or bytes(32), ikm, hashlib.sha256).digest()


def expand(prk, info, length):
    out, block, counter = b"", b"", 1
    while len(out) < length:
        block = hmac.new(prk, block + info + bytes([counter]), hashlib.sha256).digest()
        out += block
        counter += 1
    return out[:length]


def labeled_extract(suite, salt, label, ikm):
    return extract(salt, b"HPKE-v1" + suite + label + ikm)


def labeled_expand(suite, prk, label, info, length):
    return expand(prk, length.to_bytes(2, "big") + b"HPKE-v1" + suite + label + info, length)


def derive_keypair(ikm):
    dkp_prk = labeled_extract(KEM_SUITE, b"", b"dkp_prk", ikm)
    sk = labeled_expand(KEM_SUITE, dkp_prk, b"sk", b"", 32)
    return sk, X25519PrivateKey.from_private_bytes(sk).public_key().public_bytes(*RAW)


def encap(pk_r, ikm_e):
    sk_e, enc = derive_keypair(ikm_e)
    dh = X25519PrivateKey.from_private_bytes(sk_e).exchange(X25519PublicKey.from_public_bytes(pk_r))
    eae_prk = labeled_extract(KEM_SUITE, b"", b"eae_prk", dh)
    shared = labeled_expand(KEM_SUITE, eae_prk, b"shared_secret", enc + pk_r, 32)
    return shared, enc


def key_schedule_base(shared, info):
    psk_id_hash = labeled_extract(HPKE_SUITE, b"", b"psk_id_hash", b"")
    info_hash = labeled_extract(HPKE_SUITE, b"", b"info_hash", info)
    context = bytes([0]) + psk_id_hash + info_hash
    secret = labeled_extract(HPKE_SUITE, shared, b"secret", b"")
    key = labeled_expand(HPKE_SUITE, secret, b"key", context, 32)
    base_nonce = labeled_expand(HPKE_SUITE, secret, b"base_nonce", context, 12)
    return key, base_nonce


def seal_base(pk_r, info, aad, plaintext, ikm_e):
    shared, enc = encap(pk_r, ikm_e)
    key, base_nonce = key_schedule_base(shared, info)
    # Sequence number 0: the nonce is the base nonce.
    return enc, ChaCha20Poly1305(key).encrypt(base_nonce, plaintext, aad)


# RFC 9180, A.2.1 and A.2.1.1, sequence number 0.
h = bytes.fromhex
A2_IKM_R = h("1ac01f181fdf9f352797655161c58b75c656a6cc2716dcb66372da835542e1df")
A2_PK_R = h("4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a")
A2_IKM_E = h("909a9b35d3dc4713a5e72a4da274b55d3d3821a37e5d099e74a647db583a904b")
A2_ENC = h("1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a")
A2_INFO = h("4f6465206f6e2061204772656369616e2055726e")
A2_PT = h("4265617574792069732074727574682c20747275746820626561757479")
A2_AAD = h("436f756e742d30")
A2_CT = h(
    "1c5250d8034ec2b784ba2cfd69dbdb8af406cfe3ff938e131f0def8c8b60b4db"
    "21993c62ce81883d2dd1b51a28"
)
assert derive_keypair(A2_IKM_R)[1] == A2_PK_R, "DeriveKeyPair differs from RFC 9180 A.2.1"
assert seal_base(A2_PK_R, A2_INFO, A2_AAD, A2_PT, A2_IKM_E) == (A2_ENC, A2_CT), (
    "SealBase differs from RFC 9180 A.2.1.1"
)

# The envelope's inputs. The recipient's secrets are those of the signed,
# sealed change's member in generate.py.
SPACE = bytes(range(16))
GENERATION = (1 << 48) | 0xA1B2C3D4E5F6  # ordinal 1, a 48-bit tag
SPACE_KEY = bytes(0xC0 + i for i in range(32))
SIGNING_SEED = bytes(0x10 + i for i in range(32))
AGREEMENT_SECRET = bytes(0x60 + i for i in range(32))
IKM_E = bytes(0x90 + i for i in range(32))

signing_public = Ed25519PrivateKey.from_private_bytes(SIGNING_SEED).public_key().public_bytes(*RAW)
agreement_private = X25519PrivateKey.from_private_bytes(AGREEMENT_SECRET)
agreement_public = agreement_private.public_key().public_bytes(*RAW)
member_id = hashlib.sha256(b"nocentraldb/member-id" + signing_public + agreement_public).digest()

info = b"nocentraldb/key-envelope" + SPACE + GENERATION.to_bytes(8, "big") + member_id
enc, ciphertext = seal_base(agreement_public, info, b"", SPACE_KEY, IKM_E)
assert len(enc) == 32 and len(ciphertext) == 48

# A second implementation opens it: cryptography's HPKE takes enc || ct.
suite = hpke.Suite(hpke.KEM.X25519, hpke.KDF.HKDF_SHA256, hpke.AEAD.CHACHA20_POLY1305)
assert suite.decrypt(enc + ciphertext, agreement_private, info=info) == SPACE_KEY

fields = cbor2.dumps({8: member_id, 9: GENERATION, 10: enc, 11: ciphertext}, canonical=True)


def write(name, data):
    text = data.hex()
    lines = [text[i : i + 64] for i in range(0, len(text), 64)]
    (HERE / f"{name}.hex").write_text("\n".join(lines) + "\n")


write("key-envelope.member-id", member_id)
write("key-envelope.info", info)
write("key-envelope.enc", enc)
write("key-envelope.ciphertext", ciphertext)
write("key-envelope.fields", fields)
