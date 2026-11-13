// Holds RFC 0003's key record, format 2 (Carsten, 2026-11-12, NOC-285): the
// member's two secrets, an account byte for a database waiting to be
// paired, one that holds the account key, or an agent, and the recovery
// code until the person stored it; format 1 is still read. The committed
// vectors in docs/rfc/vectors/0003, which key_record.py writes with
// Python, decode as the layout says, and their signatures verify, here in
// Node, independently of the generator and of the SDKs. RFC 0010 gives the
// number of a recovered account key and what an existing database gets.
import assert from "node:assert/strict";
import { createHash, createPublicKey, verify } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { read, section } from "./lib/rfc-text.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0010 = "docs/rfc/0010-accounts.md";
const VECTORS = "docs/rfc/vectors/0003";

const file = (name) => readFileSync(join(root, VECTORS, name), "utf8");
const hex = (name) => Buffer.from(file(name).replace(/\s+/g, ""), "hex");
const record = (state) => hex(`key-record.${state}.hex`);

/** The fields of key-record.fields.txt, by name, as bytes. */
const fields = () =>
  Object.fromEntries(
    file("key-record.fields.txt")
      .trim()
      .split("\n")
      .map((line) => {
        const [name, value] = line.split(": ");
        return [name, Buffer.from(value, "hex")];
      }),
  );

const sha256 = (...parts) => {
  const hash = createHash("sha256");
  for (const part of parts) hash.update(part);
  return hash.digest();
};

/** The canonical CBOR map {1: a, 2: b, 3: n} of 32-byte strings a, b and
 * a small unsigned n, as a statement or a certificate signs it. */
const map3 = (a, b, n) => {
  assert.ok(n < 24);
  const bstr = (bytes) => Buffer.concat([Buffer.from([0x58, 32]), bytes]);
  return Buffer.concat([
    Buffer.from([0xa3, 0x01]),
    bstr(a),
    Buffer.from([0x02]),
    bstr(b),
    Buffer.from([0x03, n]),
  ]);
};

/** Whether `signature` by the Ed25519 public key `key` verifies `message`. */
const holds = (key, message, signature) =>
  verify(
    null,
    message,
    createPublicKey({
      key: Buffer.concat([Buffer.from("302a300506032b6570032100", "hex"), key]),
      format: "der",
      type: "spki",
    }),
    signature,
  );

const MAGIC = Buffer.from("nocentraldb-keys");

test("RFC 0003 gives the key record's format 2 and still reads format 1", () => {
  const part = section(read(RFC_0003), "the-member-key");
  for (const pattern of [
    /\*\*Format 2\*\* \(Carsten, 2026-11-12, NOC-285\)/,
    /\| 16 \| 1 \| format \| `2` \|/,
    /\| 81 \| 1 \| account \|/,
    /`0`: no account yet/,
    /`1`: the database holds the account key/,
    /`2`: an agent, which holds only its certificate/,
    /83 bytes waiting to be paired, 219 with the account key, or 235 while it also holds the code, and 283 for an agent/,
    /\*\*Format 1\*\*, 113 bytes/,
    /still reads it and writes the record again as format 2 on the next open/,
    /key_record\.py/,
    /28 bytes more than the record/,
  ]) {
    assert.match(
      part,
      pattern,
      `RFC 0003's member key does not say ${pattern}`,
    );
  }
});

test("RFC 0010 gives the number of a recovered account key and what an existing database gets", () => {
  const part = section(read(RFC_0010), "the-recovery-code");
  for (const pattern of [
    /one more than the highest number the device has learned, and the current time in Unix seconds \(Carsten, 2026-11-12, NOC-285\)/,
    /\*\*Databases that exist today\.\*\*/,
    /recovery code and statement number 1 for the account key it holds/,
    /entries already written stay as written/,
  ]) {
    assert.match(
      part,
      pattern,
      `RFC 0010's recovery code does not say ${pattern}`,
    );
  }
  assert.match(
    section(read(RFC_0010), "adding-a-device-or-an-agent-to-an-account"),
    /version 2 of \[RFC 0003's invitation\]\(0003-keys-and-cryptography\.md#invitation\)/,
  );
});

test("each record is the magic, its format, and the member's secrets", () => {
  const f = fields();
  for (const state of [
    "format-1",
    "pairing",
    "device",
    "device-code",
    "agent",
  ]) {
    const bytes = record(state);
    assert.deepEqual(bytes.subarray(0, 16), MAGIC, state);
    assert.equal(bytes[16], state === "format-1" ? 1 : 2, state);
    assert.deepEqual(bytes.subarray(17, 49), f["member signing secret"], state);
    assert.deepEqual(
      bytes.subarray(49, 81),
      f["member agreement secret"],
      state,
    );
  }
  const old = record("format-1");
  assert.equal(old.length, 113);
  assert.deepEqual(old.subarray(81, 113), f["account secret"]);
});

test("a record waiting to be paired has no account and no code", () => {
  const bytes = record("pairing");
  assert.equal(bytes.length, 83);
  assert.equal(bytes[81], 0, "no account yet");
  assert.equal(bytes[82], 0, "no code");
});

test("a record with the account key holds its statement, and the code until confirmed", () => {
  const f = fields();
  for (const [state, length] of [
    ["device", 219],
    ["device-code", 235],
  ]) {
    const bytes = record(state);
    assert.equal(bytes.length, length, state);
    assert.equal(bytes[81], 1, "the account key");
    assert.deepEqual(bytes.subarray(82, 114), f["account secret"]);
    assert.deepEqual(bytes.subarray(114, 146), f["recovery key"]);
    assert.deepEqual(bytes.subarray(146, 154), f.number);
    assert.deepEqual(bytes.subarray(154, 218), f["statement signature"]);
  }
  assert.equal(record("device")[218], 0, "no code");
  assert.equal(record("device-code")[218], 1, "the code follows");
  assert.deepEqual(
    record("device-code").subarray(219, 235),
    f["recovery code"],
  );
});

test("an agent's record holds its certificate and statement, and no account secret", () => {
  const f = fields();
  const bytes = record("agent");
  assert.equal(bytes.length, 283);
  assert.equal(bytes[81], 2, "an agent");
  assert.deepEqual(bytes.subarray(82, 114), f["account key"]);
  assert.deepEqual(bytes.subarray(114, 178), f["agent certificate signature"]);
  assert.deepEqual(bytes.subarray(178, 210), f["recovery key"]);
  assert.deepEqual(bytes.subarray(210, 218), f.number);
  assert.deepEqual(bytes.subarray(218, 282), f["statement signature"]);
  assert.equal(bytes[282], 0, "no code");
  assert.ok(!bytes.includes(f["account secret"]), "no account secret");
});

test("the statement verifies against the recovery key, and the agent's certificate against the account key", () => {
  const f = fields();
  const number = Number(f.number.readBigUInt64BE());
  assert.ok(
    holds(
      f["recovery key"],
      sha256(
        "nocentraldb/account-key-statement",
        map3(f["recovery key"], f["account key"], number),
      ),
      f["statement signature"],
    ),
    "the statement",
  );
  assert.ok(
    holds(
      f["account key"],
      sha256(
        "nocentraldb/member-cert",
        map3(f["account key"], f["member id"], 2),
      ),
      f["agent certificate signature"],
    ),
    "the agent's certificate",
  );
});
