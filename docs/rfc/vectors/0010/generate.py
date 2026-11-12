"""Writes the test vectors of RFC 0010's recovery code
(docs/rfc/0010-accounts.md, "The recovery code" and "The fingerprint") and
RFC 0003's recovery key ("Recovery key") in this directory, independently of
the Rust core and the SDKs: the BIP-39 encoding by hand from the word list,
hashlib for SHA-256, cryptography's HKDF and Ed25519, and cbor2 (6.2.1,
canonical mode) for the account key statement.

The word list is the BIP-39 English list the core embeds,
crates/ncdb-core/src/recovery/bip39-english.txt; its SHA-256 is checked
first. The encoding is checked against three vectors of BIP-39's own
vectors.json (the 128-bit ones of 0x00, 0x7f and 0xff); BIP-39's PBKDF2
seed step is not used.

Each code is named by its bits, and has five files:
  <name>.bits.hex          the 128 bits
  <name>.words.txt         the twelve words, separated by one space
  <name>.recovery-key.hex  the recovery key's Ed25519 public key
  <name>.fingerprint.txt   the fingerprint over it: 60 digits in twelve
                           blocks of five
  <name>.statement.hex     the first account key statement (number 1),
                           signed by the recovery key, for the account key
                           of the seed 0x11 * 32 (as in docs/rfc/vectors/0003)

The gates do not run this script; crates/ncdb-core/tests/recovery_code.rs
checks the committed files.

Run: python3 -I generate.py  (needs cbor2==6.2.1 and cryptography==50.0.x;
Ed25519 signatures are deterministic, so a patch version changes no byte)
"""

import hashlib
import pathlib

import cbor2
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

HERE = pathlib.Path(__file__).parent
LIST = HERE / "../../../../crates/ncdb-core/src/recovery/bip39-english.txt"
LIST_SHA256 = "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"


def sha256(data):
    return hashlib.sha256(data).digest()


def raw(public_key):
    return public_key.public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )


data = LIST.resolve().read_bytes()
assert hashlib.sha256(data).hexdigest() == LIST_SHA256, "not the BIP-39 English list"
WORDS = data.decode().split("\n")[:-1]
assert len(WORDS) == 2048 and WORDS == sorted(WORDS)


def words(bits):
    """BIP-39: the 128 bits, then the first 4 bits of their SHA-256, read in
    11-bit groups, each an index into the list."""
    assert len(bits) == 16
    number = int.from_bytes(bits, "big") << 4 | sha256(bits)[0] >> 4
    return [WORDS[(number >> (11 * (11 - i))) & 0x7FF] for i in range(12)]


# BIP-39's own vectors (vectors.json, the 128-bit ones).
assert " ".join(words(bytes(16))) == " ".join(["abandon"] * 11 + ["about"])
assert " ".join(words(bytes([0x7F] * 16))) == (
    "legal winner thank year wave sausage worth useful legal winner thank yellow"
)
assert " ".join(words(bytes([0xFF] * 16))) == " ".join(["zoo"] * 11 + ["wrong"])


def recovery(bits):
    """RFC 0003, "Recovery key": the Ed25519 seed is HKDF-SHA256 of the bits,
    no salt, info "nocentraldb/recovery-key"."""
    seed = HKDF(
        algorithm=hashes.SHA256(), length=32, salt=None, info=b"nocentraldb/recovery-key"
    ).derive(bits)
    return Ed25519PrivateKey.from_private_bytes(seed)


def fingerprint(recovery_public):
    """RFC 0010, "The fingerprint"."""
    digest = sha256(b"nocentraldb/account-fingerprint" + recovery_public)
    digits = f"{int.from_bytes(digest, 'big') % 10**60:060d}"
    return " ".join(digits[i : i + 5] for i in range(0, 60, 5))


ACCOUNT_PUBLIC = raw(Ed25519PrivateKey.from_private_bytes(bytes([0x11] * 32)).public_key())


def statement(key, number):
    """RFC 0003, "Account key statement": the canonical map of keys 1 to 4,
    the signature over the SHA-256 hash of the context and keys 1 to 3."""
    public = raw(key.public_key())
    unsigned = {1: public, 2: ACCOUNT_PUBLIC, 3: number}
    signature = key.sign(
        sha256(b"nocentraldb/account-key-statement" + cbor2.dumps(unsigned, canonical=True))
    )
    return cbor2.dumps({**unsigned, 4: signature}, canonical=True)


CODES = {
    "zero": bytes(16),
    "7f": bytes([0x7F] * 16),
    "a0": bytes(0xA0 + i for i in range(16)),
    "ff": bytes([0xFF] * 16),
}

for name, bits in CODES.items():
    key = recovery(bits)
    public = raw(key.public_key())
    files = {
        "bits.hex": bits.hex(),
        "words.txt": " ".join(words(bits)),
        "recovery-key.hex": public.hex(),
        "fingerprint.txt": fingerprint(public),
        "statement.hex": statement(key, 1).hex(),
    }
    for suffix, text in files.items():
        (HERE / f"{name}.{suffix}").write_text(text + "\n")
