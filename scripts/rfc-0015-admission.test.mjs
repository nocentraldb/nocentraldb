// Holds RFC 0015's admission ("The admission"; NOC-469): a canonical CBOR
// map of the relay id, the identity, until and spaces, signed as key 5 by
// the admission key over SHA-256 of "nocentraldb/relay-admission" and the
// map of keys 1 to 4, and as text `ncdb-admit-1:` with unpadded base64url.
// The committed vectors in docs/rfc/vectors/0015, which generate.py writes
// with Python, decode and verify here with Node's own Ed25519 and SHA-256,
// independently of the generator and of the core.
import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { read, root, section } from "./lib/rfc-text.mjs";

const RFC_0015 = "docs/rfc/0015-relay-admission.md";
const VECTORS = "docs/rfc/vectors/0015";
const PREFIX = "ncdb-admit-1:";
const CONTEXT = "nocentraldb/relay-admission";

const file = (name) => readFileSync(join(root, VECTORS, name), "utf8").trim();
const hex = (name) => Buffer.from(file(name), "hex");

/** The fields of fields.txt, by name, as bytes. */
const fields = () =>
  Object.fromEntries(
    file("fields.txt")
      .split("\n")
      .map((line) => {
        const [name, value] = line.split(": ");
        return [name, Buffer.from(value, "hex")];
      }),
  );

/** An Ed25519 public key from its 32 raw bytes. */
const ed25519 = (raw) =>
  createPublicKey({
    key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), raw]),
    format: "der",
    type: "spki",
  });

/** CBOR's head of a byte string of `length` bytes, below 256. */
const bytesHead = (length) =>
  length < 24 ? Buffer.from([0x40 + length]) : Buffer.from([0x58, length]);

/** CBOR's head of an unsigned integer below 2^32. */
const unsigned = (n) => {
  if (n < 24) return Buffer.from([n]);
  if (n < 0x100) return Buffer.from([0x18, n]);
  if (n < 0x10000) return Buffer.from([0x19, n >> 8, n & 0xff]);
  const out = Buffer.alloc(5);
  out[0] = 0x1a;
  out.writeUInt32BE(n, 1);
  return out;
};

/** The canonical map of keys 1 to 4, built by hand. */
const unsignedMap = ({ relayId, identity, until, spaces }) =>
  Buffer.concat([
    Buffer.from([0xa4, 0x01]),
    bytesHead(relayId.length),
    relayId,
    Buffer.from([0x02]),
    bytesHead(identity.length),
    identity,
    Buffer.from([0x03]),
    unsigned(until),
    Buffer.from([0x04]),
    unsigned(spaces),
  ]);

const sha256 = (...parts) => {
  const hash = createHash("sha256");
  for (const part of parts) hash.update(part);
  return hash.digest();
};

for (const [name, identityField, digestField, spaces] of [
  ["admission", "recovery key", "digest", 20],
  ["admission-account-key", "account key", "account key digest", 1],
]) {
  test(`the vector ${name} is the map RFC 0015 gives, signed by the admission key`, () => {
    const f = fields();
    const until = Number(f.until.readBigUInt64BE());
    const map = unsignedMap({
      relayId: f["relay id"],
      identity: f[identityField],
      until,
      spaces,
    });
    const digest = sha256(Buffer.from(CONTEXT), map);
    assert.deepEqual(digest, f[digestField]);
    const bytes = hex(`${name}.hex`);
    const signature = bytes.subarray(bytes.length - 64);
    const signed = Buffer.concat([
      Buffer.from([0xa5]),
      map.subarray(1),
      Buffer.from([0x05, 0x58, 0x40]),
      signature,
    ]);
    assert.deepEqual(bytes, signed, "keys 1 to 4, then the signature as key 5");
    assert.ok(verify(null, digest, ed25519(f["admission key"]), signature));
  });

  test(`the text of ${name} is the prefix and unpadded base64url of its bytes`, () => {
    const text = file(`${name}.txt`);
    assert.ok(text.startsWith(PREFIX));
    const body = text.slice(PREFIX.length);
    assert.match(body, /^[A-Za-z0-9_-]+$/, "unpadded base64url");
    assert.deepEqual(Buffer.from(body, "base64url"), hex(`${name}.hex`));
  });
}

test("RFC 0015's admission points to its vectors", () => {
  const part = section(read(RFC_0015), "the-admission");
  assert.match(part, /docs\/rfc\/vectors\/0015\//);
  assert.match(part, /generate\.py/);
});
