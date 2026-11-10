// Checks the byte-exact examples of RFC 0004, "Byte-exact examples": E1 to
// E14 (NOC-37), which NOC-16 and NOC-17 build their encoders from, and E15
// to E17 (NOC-63, the reference): every example's hex in the
// RFC decodes as canonical CBOR under RFC 0002's profile (read strictly and
// re-encoded to the same bytes), and equals the committed vector that
// docs/rfc/vectors/0004/generate.py writes with Python's cbor2, independently
// of this reader and of the Rust core. The gates do not run the generator;
// they check the committed vectors.
import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import {
  decode,
  encode,
  fromHex,
  MAX_DEPTH,
  ProfileError,
  toHex,
} from "./lib/cbor-profile.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const RFC = "docs/rfc/0004-merge-rules-basic-types.md";
const VECTORS = "docs/rfc/vectors/0004";

/** An operation map sits at depth 5: frame, header, payload, operations. */
const OPERATION_DEPTH = 5;

/** The examples of the RFC, by name: { E1: "a501…", … }. */
const examples = () => {
  const text = readFileSync(join(root, RFC), "utf8");
  const sections = text
    .split(/^### Byte-exact examples\b.*$/m)
    .slice(1)
    .map((rest) => rest.split(/^#{1,3} /m)[0]);
  const found = {};
  for (const part of sections.flatMap((section) =>
    section.split(/^\*\*(?=E\d+\.)/m).slice(1),
  )) {
    const name = part.match(/^(E\d+)\./)[1];
    const hex = [...part.matchAll(/```text\n((?:[0-9a-f]+\n)+)```/g)];
    assert.equal(
      hex.length,
      1,
      `${RFC}: ${name} has ${hex.length} hex blocks, not 1`,
    );
    assert.ok(!(name in found), `${RFC}: ${name} appears twice`);
    found[name] = hex[0][1].replace(/\s+/g, "");
  }
  return found;
};

test("the RFC has the examples E1 to E17", () => {
  assert.deepEqual(
    Object.keys(examples()),
    Array.from({ length: 17 }, (_, i) => `E${i + 1}`),
  );
});

test("every example is canonical CBOR under RFC 0002's profile", () => {
  for (const [name, hex] of Object.entries(examples())) {
    const bytes = fromHex(hex);
    const value = decode(bytes, OPERATION_DEPTH);
    assert.equal(
      toHex(encode(value)),
      hex,
      `${name} does not re-encode to its bytes`,
    );
  }
});

test("every example equals the committed vector of the generator", () => {
  const rfc = examples();
  const files = existsSync(join(root, VECTORS))
    ? readdirSync(join(root, VECTORS)).filter((file) => file.endsWith(".hex"))
    : [];
  assert.deepEqual(
    files.map((file) => file.replace(/\.hex$/, "")).sort(),
    Object.keys(rfc).sort(),
    `${VECTORS} holds no vector for some example, or one the RFC lacks`,
  );
  for (const [name, hex] of Object.entries(rfc)) {
    const vector = readFileSync(join(root, VECTORS, `${name}.hex`), "utf8");
    assert.equal(
      hex,
      vector.replace(/\s+/g, ""),
      `${name} differs from ${VECTORS}/${name}.hex`,
    );
  }
});

// The reader must refuse what the profile forbids, or the check above
// proves nothing (GENESIS.md: fixtures that prove detectors fire).
test("the profile reader refuses every kind of non-canonical input", () => {
  const refused = {
    "indefinite length": "9f01ff",
    "integer not in its shortest form": "1801",
    "length not in its shortest form": "5800",
    "map keys not sorted": "a202010101",
    "map keys repeated": "a201010102",
    "negative map key": "a12001",
    "byte string as map key": "a1410001",
    tag: "c101",
    "simple value undefined": "f7",
    "invalid UTF-8": "61ff",
    "bytes after the value": "0101",
    truncated: "42ff",
  };
  for (const [reason, hex] of Object.entries(refused)) {
    assert.throws(
      () => decode(fromHex(hex)),
      ProfileError,
      `accepted ${reason}`,
    );
  }
  const deep = `${"81".repeat(MAX_DEPTH - OPERATION_DEPTH + 1)}00`;
  assert.throws(() => decode(fromHex(deep), OPERATION_DEPTH), ProfileError);
  assert.doesNotThrow(() => decode(fromHex(deep.slice(2)), OPERATION_DEPTH));
});

test("re-encoding catches a float that is not in its shortest form, or another NaN", () => {
  for (const hex of [
    "fa3f800000",
    "fb3ff0000000000000",
    "f97e01",
    "fa7fc00000",
  ]) {
    assert.notEqual(
      toHex(encode(decode(fromHex(hex)))),
      hex,
      `${hex} re-encodes to itself`,
    );
  }
  for (const hex of [
    "f93c00",
    "f98000",
    "f97e00",
    "fa47c35000",
    "fb3ff199999999999a",
  ]) {
    assert.equal(
      toHex(encode(decode(fromHex(hex)))),
      hex,
      `${hex} does not re-encode to itself`,
    );
  }
});
