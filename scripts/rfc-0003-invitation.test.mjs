// Holds RFC 0003's invitation (NOC-180; the owner's NOC-141 A, Carsten,
// 2026-11-10): the text `ncdb-invite-1:` plus unpadded base64url of a fixed
// binary layout that starts with a version byte and holds only public data.
// RFC 0003 gives the layout field by field and what a reader refuses; RFC
// 0009 points to it; and the committed vectors in docs/rfc/vectors/0003,
// which generate.py writes with Python, decode as the layout says, here in
// Node, independently of the generator and of the SDKs. Version 2, the
// invitation of a member waiting to be paired, has no certificate (Carsten,
// 2026-11-12, NOC-285).
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { read, section } from "./lib/rfc-text.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC_0003 = "docs/rfc/0003-keys-and-cryptography.md";
const RFC_0009 = "docs/rfc/0009-sdk-api.md";
const VECTORS = "docs/rfc/vectors/0003";
const PREFIX = "ncdb-invite-1:";

const file = (name) => readFileSync(join(root, VECTORS, name), "utf8");
const hex = (name) => Buffer.from(file(name).replace(/\s+/g, ""), "hex");

/** The fields of fields.txt, by name, as bytes. */
const fields = () =>
  Object.fromEntries(
    file("fields.txt")
      .trim()
      .split("\n")
      .map((line) => {
        const [name, value] = line.split(": ");
        return [name, Buffer.from(value, "hex")];
      }),
  );

/** The bytes of an invitation's text form, as RFC 0003 reads it. */
const decode = (text) => {
  assert.ok(text.startsWith(PREFIX), "the text starts with the prefix");
  const body = text.slice(PREFIX.length);
  assert.match(body, /^[A-Za-z0-9_-]+$/, "unpadded base64url");
  return Buffer.from(body, "base64url");
};

test("RFC 0003 gives the invitation's text form and byte layout", () => {
  const part = section(read(RFC_0003), "invitation");
  for (const pattern of [
    /`ncdb-invite-1:`/,
    /base64url/,
    /without padding/,
    /\| 0 \| 1 \| version \|/,
    /\| 1 \| 32 \| signing key \|/,
    /\| 33 \| 32 \| agreement key \|/,
    /\| 65 \| 32 \| account key \|/,
    /\| 97 \| 1 \| kind \|/,
    /\| 98 \| 64 \| certificate signature \|/,
    /\| 162 \| 1 \| statement \|/,
    /\| 163 \| 32 \| recovery key \|/,
    /\| 195 \| 8 \| number \|/,
    /\| 203 \| 64 \| statement signature \|/,
    /163 bytes/,
    /267 bytes/,
    /`invalidValue`/,
    /NOC-141/,
    /vectors\/0003/,
  ]) {
    assert.match(
      part,
      pattern,
      `RFC 0003's Invitation does not say ${pattern}`,
    );
  }
});

test("RFC 0009 points invitation.toString() and nc.invitation(text) to RFC 0003's layout", () => {
  const rfc = read(RFC_0009);
  assert.match(
    section(rfc, "invitations-and-members"),
    /0003-keys-and-cryptography\.md#invitation/,
  );
  assert.match(
    section(rfc, "the-rust-sdk"),
    /`Display`[^.]*`ncdb-invite-1:`|`ncdb-invite-1:`[^.]*`Display`/,
  );
  assert.match(section(rfc, "the-rust-sdk"), /`FromStr`/);
});

test("the vectors' text and bytes are one invitation each", () => {
  for (const name of [
    "invitation",
    "invitation-statement",
    "invitation-pairing",
  ]) {
    assert.deepEqual(decode(file(`${name}.txt`).trim()), hex(`${name}.hex`));
  }
});

test("the vector without a statement decodes field by field as the layout says", () => {
  const bytes = hex("invitation.hex");
  const f = fields();
  assert.equal(bytes.length, 163);
  assert.equal(bytes[0], 1, "version 1");
  assert.deepEqual(bytes.subarray(1, 33), f["signing key"]);
  assert.deepEqual(bytes.subarray(33, 65), f["agreement key"]);
  assert.deepEqual(bytes.subarray(65, 97), f["account key"]);
  assert.deepEqual(bytes.subarray(97, 98), f.kind);
  assert.deepEqual(bytes.subarray(98, 162), f["certificate signature"]);
  assert.equal(bytes[162], 0, "no statement");
  const id = createHash("sha256")
    .update("nocentraldb/member-id")
    .update(f["signing key"])
    .update(f["agreement key"])
    .digest();
  assert.deepEqual(id, f["member id"], "the member id derives from the keys");
});

test("the vector with a statement adds the recovery key, the number and its signature", () => {
  const bytes = hex("invitation-statement.hex");
  const f = fields();
  assert.equal(bytes.length, 267);
  assert.deepEqual(
    bytes.subarray(0, 162),
    hex("invitation.hex").subarray(0, 162),
  );
  assert.equal(bytes[162], 1, "a statement");
  assert.deepEqual(bytes.subarray(163, 195), f["recovery key"]);
  assert.deepEqual(bytes.subarray(195, 203), f.number);
  assert.deepEqual(bytes.subarray(203, 267), f["statement signature"]);
});

test("RFC 0003 gives version 2, the invitation of a member waiting to be paired", () => {
  const part = section(read(RFC_0003), "invitation");
  for (const pattern of [
    /\*\*Version 2: a member waiting to be paired\.\*\*/,
    /\| 0 \| 1 \| version \| `2` \|/,
    /\| 65 \| 1 \| kind \|/,
    /66 bytes/,
    /`members\.add` refuses it with `invalidCertificate`/,
    /Version 1 stays as it is/,
    /NOC-285/,
  ]) {
    assert.match(
      part,
      pattern,
      `RFC 0003's Invitation does not say ${pattern}`,
    );
  }
});

test("the version 2 vector holds the keys and the kind and no certificate", () => {
  const bytes = hex("invitation-pairing.hex");
  const f = fields();
  assert.equal(bytes.length, 66);
  assert.equal(bytes[0], 2, "version 2");
  assert.deepEqual(bytes.subarray(1, 33), f["signing key"]);
  assert.deepEqual(bytes.subarray(33, 65), f["agreement key"]);
  assert.deepEqual(bytes.subarray(65, 66), f.kind);
});
