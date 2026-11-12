// Holds the owner's answers on NOC-244 (Carsten, 2026-11-12: A to all twelve
// questions) as the RFCs record them, and the committed vectors of the sealed
// personal value in docs/rfc/vectors/0003, which personal_value.py writes with
// cbor2, cryptography and PyNaCl. This test checks those bytes again with
// Node's own primitives: the associated data from its parts, the subkey with
// HKDF-SHA256, and the ciphertext by opening it with XChaCha20-Poly1305 built
// from Node's ChaCha20-Poly1305 and HChaCha20. Whether the design is right is
// the owner's decision, not this test's.
import assert from "node:assert/strict";
import { createDecipheriv, hkdfSync } from "node:crypto";
import { test } from "node:test";
import { read, section } from "./lib/rfc-text.mjs";

const VECTORS = "docs/rfc/vectors/0003";
const SOURCE = "(Carsten, 2026-11-12, NOC-244)";
const RFC = {
  "0002": "docs/rfc/0002-change-and-log-format.md",
  "0003": "docs/rfc/0003-keys-and-cryptography.md",
  "0004": "docs/rfc/0004-merge-rules-basic-types.md",
  "0005": "docs/rfc/0005-schema-and-model-lock.md",
  "0009": "docs/rfc/0009-sdk-api.md",
};

const hex = (file) =>
  Buffer.from(read(`${VECTORS}/${file}`).replace(/\s/g, ""), "hex");
const range = (start, length) =>
  Buffer.from(Array.from({ length }, (_, i) => start + i));
const name = (text) => {
  const bytes = Buffer.from(text, "utf8");
  const length = Buffer.alloc(2);
  length.writeUInt16BE(bytes.length);
  return Buffer.concat([length, bytes]);
};

const LABEL = Buffer.from("nocentraldb/personal-value");
const SPACE = range(0x10, 16);
const OBJECT = range(0x30, 16);
const PERSON_KEY_ID = range(0x61, 16);
const PERSON_KEY = Buffer.alloc(32, 0x55);

/** HChaCha20 (draft-irtf-cfrg-xchacha, 2.2): the subkey for a 16-byte nonce. */
const hchacha20 = (key, nonce) => {
  const rotl = (v, c) => ((v << c) | (v >>> (32 - c))) >>> 0;
  const s = new Uint32Array(16);
  s.set([0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]);
  for (let i = 0; i < 8; i++) s[4 + i] = key.readUInt32LE(4 * i);
  for (let i = 0; i < 4; i++) s[12 + i] = nonce.readUInt32LE(4 * i);
  const quarter = (a, b, c, d) => {
    s[a] = (s[a] + s[b]) >>> 0;
    s[d] = rotl(s[d] ^ s[a], 16);
    s[c] = (s[c] + s[d]) >>> 0;
    s[b] = rotl(s[b] ^ s[c], 12);
    s[a] = (s[a] + s[b]) >>> 0;
    s[d] = rotl(s[d] ^ s[a], 8);
    s[c] = (s[c] + s[d]) >>> 0;
    s[b] = rotl(s[b] ^ s[c], 7);
  };
  for (let round = 0; round < 10; round++) {
    quarter(0, 4, 8, 12);
    quarter(1, 5, 9, 13);
    quarter(2, 6, 10, 14);
    quarter(3, 7, 11, 15);
    quarter(0, 5, 10, 15);
    quarter(1, 6, 11, 12);
    quarter(2, 7, 8, 13);
    quarter(3, 4, 9, 14);
  }
  const out = Buffer.alloc(32);
  [0, 1, 2, 3, 12, 13, 14, 15].forEach((w, i) => {
    out.writeUInt32LE(s[w], 4 * i);
  });
  return out;
};

/** Opens nonce (24) || ciphertext || tag (16) with XChaCha20-Poly1305. */
const open = (key, sealed, ad) => {
  const nonce = sealed.subarray(0, 24);
  const tag = sealed.subarray(sealed.length - 16);
  const body = sealed.subarray(24, sealed.length - 16);
  const subkey = hchacha20(key, nonce.subarray(0, 16));
  const iv = Buffer.concat([Buffer.alloc(4), nonce.subarray(16)]);
  const decipher = createDecipheriv("chacha20-poly1305", subkey, iv, {
    authTagLength: 16,
  });
  decipher.setAAD(ad, { plaintextLength: body.length });
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(body), decipher.final()]);
};

test("the associated data is the context, the three ids and the two names", () => {
  const ad = Buffer.concat([
    LABEL,
    SPACE,
    OBJECT,
    PERSON_KEY_ID,
    name("people"),
    name("name"),
  ]);
  assert.deepEqual(hex("personal-value.ad.hex"), ad);
});

test("the subkey is HKDF-SHA256 of the person key with the context as info", () => {
  const subkey = Buffer.from(
    hkdfSync("sha256", PERSON_KEY, Buffer.alloc(0), LABEL, 32),
  );
  assert.deepEqual(hex("personal-value.subkey.hex"), subkey);
});

test("the sealed value opens to the canonical CBOR of the value", () => {
  const sealed = hex("personal-value.sealed.hex");
  assert.deepEqual(sealed.subarray(0, 24), range(0xc1, 24), "the nonce first");
  const plaintext = open(
    hex("personal-value.subkey.hex"),
    sealed,
    hex("personal-value.ad.hex"),
  );
  assert.deepEqual(plaintext, hex("personal-value.plaintext.hex"));
  // The text "Anna Schmidt": major type 3, length 12.
  assert.deepEqual(
    plaintext,
    Buffer.concat([Buffer.from([0x6c]), Buffer.from("Anna Schmidt")]),
  );
  // Moved to another field, it does not open.
  const moved = Buffer.concat([
    LABEL,
    SPACE,
    OBJECT,
    PERSON_KEY_ID,
    name("people"),
    name("email"),
  ]);
  assert.throws(() => open(hex("personal-value.subkey.hex"), sealed, moved));
});

test("the operations carry the sealed value, a plain null and $person in argument 1", () => {
  const operation = hex("personal-value.operation.hex");
  const sealed = hex("personal-value.sealed.hex");
  assert.ok(operation.includes(sealed), "the set carries the sealed bytes");
  assert.ok(
    operation.includes(Buffer.from([0x01, 0xa2, 0x01, 0x50])),
    "argument 1 is the map {1: person-key id, 2: sealed}",
  );
  const clear = hex("personal-value.clear.hex");
  assert.ok(clear.includes(Buffer.from([0x01, 0xf6])), "argument 1 is null");
  const person = hex("person.operation.hex");
  assert.ok(person.includes(Buffer.from("$person")));
  assert.ok(person.includes(range(0x70, 16)), "the person id");
  assert.ok(person.includes(PERSON_KEY_ID), "the person-key id");
});

test("RFC 0003 defines the sealed personal value, with the owner's decision", () => {
  const text = section(read(RFC["0003"]), "personal-value");
  for (const pattern of [
    /"nocentraldb\/personal-value"/,
    /HKDF-SHA256/,
    /2 bytes, big-endian/,
    /nonce \(24 bytes\) \|\| ciphertext \|\| tag \(16 bytes\)/,
    /canonical CBOR/,
    /personal_value\.py/,
  ]) {
    assert.match(text, pattern);
  }
  assert.ok(text.includes(SOURCE));
  assert.match(
    section(read(RFC["0003"]), "how-a-member-signs-and-encrypts-a-change"),
    /\(#personal-value\)/,
  );
});

test("RFC 0002 and RFC 0004 put the sealed value in argument 1 of a set", () => {
  assert.match(read(RFC["0002"]), /argument 1 of a set/);
  const value = read(RFC["0004"]);
  assert.doesNotMatch(value, /The personal fields RFC decides where/);
  assert.match(
    value,
    /\*\*A personal value\.\*\*[^\n]*argument 1[^\n]*NOC-244/,
  );
  assert.match(value, /\*\*A personal value\.\*\*[^\n]*plain `null`/);
});

test("RFC 0005 names the person by $person on the person's object", () => {
  const text = section(read(RFC["0005"]), "identifying-the-person");
  assert.match(text, /`\$person`/);
  assert.match(text, /person id/);
  assert.match(text, /person-key id/);
  assert.ok(text.includes(SOURCE));
});

test("RFC 0009 says how an erased field reads, and names the Rust calls", () => {
  const sdk = read(RFC["0009"]);
  const personal = section(sdk, "personal-fields");
  assert.match(personal, /`\$erased`/);
  assert.match(personal, /reads as unset/);
  assert.match(personal, /not held yet/);
  assert.match(personal, /the first person-key entry/i);
  assert.ok(personal.includes(SOURCE));
  const rust = section(sdk, "the-rust-sdk");
  assert.match(rust, /`ErrorCode::NoPerson`/);
  assert.match(rust, /`Create \{ same_person_as/);
  assert.match(rust, /`object\.erased\(\)`/);
});
