"""Writes the pairing answer test vectors of RFC 0010 ("The pairing answer";
the owner's NOC-273 A, Carsten, 2026-11-12) in this directory, independently
of the Rust core and the SDKs. The gates do not run this script;
crates/ncdb-core/tests/pairing_answer_vector.rs checks the committed files,
natively and in wasm32.

Run: python3 -I pairing_answer.py  (needs cbor2==6.2.1 and cryptography==50.0.1)

The answer is `ncdb-pair-1:` and unpadded base64url of a fixed layout:

    0    1  version, 1
    1    1  kind, 1 device or 2 agent (key 3 of the certificate)
    2   32  account key (key 1 of the certificate)
    34  64  certificate signature (key 5): the account key signs
            H("nocentraldb/member-cert" || canonical map {1: account key,
            2: member id, 3: kind})
    98  32  recovery key (key 1 of the account key statement)
    130  8  number (key 3 of the statement), big-endian, at least 1
    138 64  statement signature (key 4): the recovery key signs
            H("nocentraldb/account-key-statement" || canonical map
            {1: recovery key, 2: account key, 3: number})
    202  1  sealed key: 0 for an agent, 1 for a device, then
    203 32  HPKE's encapsulated key
    235 48  the account key's 32-byte Ed25519 seed sealed with HPKE (RFC
            0003's key envelope construction, info "nocentraldb/account-key"
            alone, no associated data)
    then 1  link: 0, or 1 and then
    then 64 the account key's half of the link statement: it signs
            H("nocentraldb/account-link" || canonical map {1: old account
            key, 2: recovery key})

HPKE (RFC 9180) base mode with RFC 0003's suite, DHKEM(X25519, HKDF-SHA256),
HKDF-SHA256 and ChaCha20-Poly1305, is written out below from cryptography's
X25519, HMAC-SHA256 and ChaCha20-Poly1305, so that the ephemeral key can come
from a fixed ikmE. It is checked against RFC 9180's vector A.2.1, and each
sealed key is opened with cryptography's own HPKE, a second implementation.

The inputs are those of crates/ncdb-core/tests/vectors/key_envelope.py: the
same recipient, the same account key and the same ikmE, so the device
answer's sealed key equals that directory's pairing.enc.hex and
pairing.ciphertext.hex. The recovery key is that of the code whose 128 bits
are all 0x42.
"""

import base64
import hashlib
import hmac
import pathlib

import cbor2
from cryptography.hazmat.primitives import hpke, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.asymmetric.x25519 import X25519PrivateKey, X25519PublicKey
from cryptography.hazmat.primitives.ciphers.aead import ChaCha20Poly1305

HERE = pathlib.Path(__file__).parent
ENVELOPE = HERE.parent.parent.parent.parent / "crates/ncdb-core/tests/vectors"
RAW = serialization.Encoding.Raw, serialization.PublicFormat.Raw

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


def seal_base(pk_r, info, plaintext, ikm_e):
    sk_e, enc = derive_keypair(ikm_e)
    dh = X25519PrivateKey.from_private_bytes(sk_e).exchange(X25519PublicKey.from_public_bytes(pk_r))
    eae_prk = labeled_extract(KEM_SUITE, b"", b"eae_prk", dh)
    shared = labeled_expand(KEM_SUITE, eae_prk, b"shared_secret", enc + pk_r, 32)
    psk_id_hash = labeled_extract(HPKE_SUITE, b"", b"psk_id_hash", b"")
    info_hash = labeled_extract(HPKE_SUITE, b"", b"info_hash", info)
    context = bytes([0]) + psk_id_hash + info_hash
    secret = labeled_extract(HPKE_SUITE, shared, b"secret", b"")
    key = labeled_expand(HPKE_SUITE, secret, b"key", context, 32)
    base_nonce = labeled_expand(HPKE_SUITE, secret, b"base_nonce", context, 12)
    return enc, ChaCha20Poly1305(key).encrypt(base_nonce, plaintext, b"")


# RFC 9180, A.2.1: the recipient's and the ephemeral key pairs derive as the
# RFC gives them. key_envelope.py checks the whole seal against A.2.1.1; the
# device answer's seal is checked against key_envelope.py's own output below.
h = bytes.fromhex
assert derive_keypair(h("1ac01f181fdf9f352797655161c58b75c656a6cc2716dcb66372da835542e1df"))[1] == h(
    "4310ee97d88cc1f088a5576c77ab0cf5c3ac797f3d95139c6c84b5429c59662a"
), "DeriveKeyPair differs from RFC 9180 A.2.1"
assert derive_keypair(h("909a9b35d3dc4713a5e72a4da274b55d3d3821a37e5d099e74a647db583a904b"))[1] == h(
    "1afa08d3dec047a643885163f1180476fa7ddb54c6a8029ea33f95796bf2ac4a"
), "the ephemeral key differs from RFC 9180 A.2.1"


def ed25519_public(seed):
    return Ed25519PrivateKey.from_private_bytes(seed).public_key().public_bytes(*RAW)


def sign(seed, context, fields):
    digest = hashlib.sha256(context + cbor2.dumps(fields, canonical=True)).digest()
    return Ed25519PrivateKey.from_private_bytes(seed).sign(digest)


# The inputs.
RECOVERY_CODE_BITS = bytes([0x42] * 16)
ACCOUNT_SEED = bytes([0x11] * 32)
OLD_ACCOUNT_SEED = bytes([0x33] * 32)
SIGNING_SEED = bytes(0x10 + i for i in range(32))
AGREEMENT_SECRET = bytes(0x60 + i for i in range(32))
IKM_E = bytes(0xB0 + i for i in range(32))
NUMBER = 2

# The recovery key's seed: HKDF-SHA256(ikm = the 16 bytes, salt = none,
# info = "nocentraldb/recovery-key") (RFC 0003, "Recovery key").
recovery_seed = expand(extract(b"", RECOVERY_CODE_BITS), b"nocentraldb/recovery-key", 32)
recovery_key = ed25519_public(recovery_seed)
account_key = ed25519_public(ACCOUNT_SEED)
old_account_key = ed25519_public(OLD_ACCOUNT_SEED)

signing_public = ed25519_public(SIGNING_SEED)
agreement_private = X25519PrivateKey.from_private_bytes(AGREEMENT_SECRET)
agreement_public = agreement_private.public_key().public_bytes(*RAW)
member_id = hashlib.sha256(b"nocentraldb/member-id" + signing_public + agreement_public).digest()

statement_signature = sign(
    recovery_seed,
    b"nocentraldb/account-key-statement",
    {1: recovery_key, 2: account_key, 3: NUMBER},
)
link_signature = sign(ACCOUNT_SEED, b"nocentraldb/account-link", {1: old_account_key, 2: recovery_key})

suite = hpke.Suite(hpke.KEM.X25519, hpke.KDF.HKDF_SHA256, hpke.AEAD.CHACHA20_POLY1305)


def answer(kind, link):
    certificate_signature = sign(
        ACCOUNT_SEED, b"nocentraldb/member-cert", {1: account_key, 2: member_id, 3: kind}
    )
    out = bytes([1, kind]) + account_key + certificate_signature
    out += recovery_key + NUMBER.to_bytes(8, "big") + statement_signature
    if kind == 1:
        enc, sealed = seal_base(agreement_public, b"nocentraldb/account-key", ACCOUNT_SEED, IKM_E)
        assert suite.decrypt(enc + sealed, agreement_private, info=b"nocentraldb/account-key") == ACCOUNT_SEED
        assert enc == h((ENVELOPE / "pairing.enc.hex").read_text().replace("\n", ""))
        assert sealed == h((ENVELOPE / "pairing.ciphertext.hex").read_text().replace("\n", ""))
        out += bytes([1]) + enc + sealed
    else:
        out += bytes([0])
    out += bytes([1]) + link_signature if link else bytes([0])
    return out


def text(data):
    return "ncdb-pair-1:" + base64.urlsafe_b64encode(data).rstrip(b"=").decode()


device = answer(1, False)
agent = answer(2, False)
device_link = answer(1, True)
assert (len(device), len(agent), len(device_link)) == (284, 204, 348)

(HERE / "pairing-answer.device.txt").write_text(text(device) + "\n")
(HERE / "pairing-answer.agent.txt").write_text(text(agent) + "\n")
(HERE / "pairing-answer.device-link.txt").write_text(text(device_link) + "\n")
